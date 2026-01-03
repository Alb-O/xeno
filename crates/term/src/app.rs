use std::io::{self, Write};
use std::time::Duration;

use termina::escape::csi::{Csi, Cursor};
use termina::event::{Event, KeyEventKind};
use termina::{PlatformTerminal, Terminal as _};
use xeno_api::Editor;
use xeno_registry::{
	HookContext, HookEventData, emit as emit_hook, emit_sync_with as emit_hook_sync_with,
};
use xeno_tui::Terminal;

use crate::backend::TerminaBackend;

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
	let mut terminal = PlatformTerminal::new()?;
	install_panic_hook(&mut terminal);
	enable_terminal_features(&mut terminal)?;
	let events = terminal.event_reader();
	let timing = RenderTiming::detect();

	let backend = TerminaBackend::new(terminal);
	let mut terminal = Terminal::new(backend)?;

	// Start UI panels (includes terminal prewarm).
	editor.ui_startup();
	emit_hook_sync_with(
		&HookContext::new(HookEventData::EditorStart, Some(&editor.extensions)),
		&mut editor.hook_runtime,
	);

	let result: io::Result<()> = async {
		loop {
			editor.ui_tick();
			editor.tick();
			editor.hook_runtime.drain().await;

			if editor.drain_command_queue().await {
				break;
			}

			if editor.take_quit_request() {
				break;
			}

			terminal.draw(|frame| editor.render(frame))?;

			// Priority: UI panel > editor mode
			let cursor_style = editor
				.ui
				.cursor_style()
				.unwrap_or_else(|| cursor_style_for_mode(editor.mode()));
			write!(
				terminal.backend_mut().terminal_mut(),
				"{}",
				Csi::Cursor(Cursor::CursorStyle(cursor_style))
			)?;
			terminal.backend_mut().terminal_mut().flush()?;

			let mut filter = |e: &Event| !e.is_escape();
			// Check needs_redraw before clearing to determine timeout
			let needs_fast_redraw = editor.needs_redraw;
			editor.needs_redraw = false;

			let timeout = if matches!(editor.mode(), xeno_base::Mode::Insert)
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
		Some(&editor.extensions),
	))
	.await;

	let terminal_inner = terminal.backend_mut().terminal_mut();
	let cleanup_result = disable_terminal_features(terminal_inner);

	result.and(cleanup_result)
}
