use std::io::{self, Write};
use std::time::Duration;

use termina::escape::csi::{Csi, Cursor};
use termina::event::{Event, KeyEventKind};
use termina::{PlatformTerminal, Terminal as _};
use xeno_editor::Editor;
use xeno_editor::hook_runtime::HookDrainBudget;
use xeno_primitives::Mode;
use xeno_registry::{
	HookContext, HookEventData, emit as emit_hook, emit_sync_with as emit_hook_sync_with,
};
use xeno_tui::Terminal;

use crate::backend::TerminaBackend;

/// Hook drain budget for fast redraw (Insert mode, panels open).
const HOOK_BUDGET_FAST: HookDrainBudget = HookDrainBudget {
	duration: Duration::from_millis(1),
	max_completions: 32,
};
/// Hook drain budget for slow redraw (Normal mode, idle).
const HOOK_BUDGET_SLOW: HookDrainBudget = HookDrainBudget {
	duration: Duration::from_millis(3),
	max_completions: 64,
};

/// Render timing configuration for frame pacing.
#[derive(Debug, Clone, Copy)]
pub struct RenderTiming {
	/// Fast render interval for responsive updates.
	pub fast: Duration,
	/// Slow render interval for background updates.
	pub slow: Duration,
}

impl RenderTiming {
	/// Detects optimal render timing for the current terminal.
	pub fn detect() -> Self {
		Self::default()
	}
}

impl Default for RenderTiming {
	fn default() -> Self {
		Self {
			fast: Duration::from_millis(16),
			slow: Duration::from_millis(50),
		}
	}
}
use crate::terminal::{
	coalesce_resize_events, cursor_style_for_mode, disable_terminal_features,
	enable_terminal_features, install_panic_hook,
};

/// Runs the editor main loop.
pub async fn run_editor(mut editor: Editor) -> io::Result<()> {
	let mut platform_terminal = PlatformTerminal::new()?;
	install_panic_hook(&mut platform_terminal);
	enable_terminal_features(&mut platform_terminal)?;
	let events = platform_terminal.event_reader();
	let timing = RenderTiming::detect();

	let backend = TerminaBackend::new(platform_terminal);
	let mut terminal = Terminal::new(backend)?;

	editor.ui_startup();
	let (extensions, hook_runtime) = editor.extensions_and_hook_runtime_mut();
	emit_hook_sync_with(
		&HookContext::new(HookEventData::EditorStart, Some(extensions)),
		hook_runtime,
	);

	let result: io::Result<()> = async {
		loop {
			editor.ui_tick();
			editor.tick();

			let hook_budget = if matches!(editor.mode(), Mode::Insert) {
				HOOK_BUDGET_FAST
			} else {
				HOOK_BUDGET_SLOW
			};
			let hook_stats = editor.hook_runtime_mut().drain_budget(hook_budget).await;
			editor
				.metrics()
				.record_hook_tick(hook_stats.completed, hook_stats.pending);

			if editor.drain_command_queue().await {
				break;
			}

			if editor.take_quit_request() {
				break;
			}

			let msg_dirty = editor.drain_messages();
			if msg_dirty.needs_redraw() {
				editor.frame_mut().needs_redraw = true;
			}

			terminal.draw(|frame| editor.render(frame))?;

			let cursor_style = editor
				.ui()
				.cursor_style()
				.unwrap_or_else(|| cursor_style_for_mode(editor.mode()));
			write!(
				terminal.backend_mut().terminal_mut(),
				"{}",
				Csi::Cursor(Cursor::CursorStyle(cursor_style))
			)?;
			terminal.backend_mut().terminal_mut().flush()?;

			let mut filter = |e: &Event| !e.is_escape();
			let needs_fast_redraw = editor.frame().needs_redraw;
			editor.frame_mut().needs_redraw = false;

			let timeout = if matches!(editor.mode(), Mode::Insert)
				|| editor.any_panel_open()
				|| needs_fast_redraw
			{
				Some(timing.fast)
			} else {
				Some(timing.slow)
			};

			let has_event = match timeout {
				Some(t) => events.poll(Some(t), &mut filter)?,
				None => true,
			};

			if !has_event {
				continue;
			}

			let event = events.read(&mut filter)?;

			match event {
				Event::Key(key)
					if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
				{
					if editor.handle_key(key).await {
						break;
					}
				}
				Event::Mouse(mouse) => {
					if editor.handle_mouse(mouse).await {
						break;
					}
				}
				Event::Paste(content) => {
					editor.handle_paste(content);
				}
				Event::WindowResized(size) => {
					let size = coalesce_resize_events(&events, size)?;
					editor.handle_window_resize(size.cols, size.rows);
				}
				Event::FocusIn => {
					editor.handle_focus_in();
				}
				Event::FocusOut => {
					editor.handle_focus_out();
				}
				_ => {}
			}
		}
		Ok(())
	}
	.await;

	emit_hook(&HookContext::new(
		HookEventData::EditorQuit,
		Some(editor.extensions()),
	))
	.await;

	let terminal_inner = terminal.backend_mut().terminal_mut();
	let cleanup_result = disable_terminal_features(terminal_inner);

	result.and(cleanup_result)
}
