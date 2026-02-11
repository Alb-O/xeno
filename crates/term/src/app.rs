use std::io::{self, Write};

use termina::escape::csi::{Csi, Cursor};
use termina::{PlatformTerminal, Terminal as _};
use xeno_editor::runtime::CursorStyle;
use xeno_editor::{Editor, TerminalConfig};
use xeno_registry::HookEventData;
use xeno_registry::hooks::{HookContext, emit as emit_hook, emit_sync_with as emit_hook_sync_with};
use xeno_tui::Terminal;

use crate::backend::TerminaBackend;
use crate::terminal::{coalesce_resize_events, disable_terminal_features_with_config, enable_terminal_features_with_config, install_panic_hook_with_config};

/// Runs the editor main loop.
pub async fn run_editor(mut editor: Editor) -> io::Result<()> {
	let mut platform_terminal = PlatformTerminal::new()?;
	let terminal_config = TerminalConfig::detect();
	install_panic_hook_with_config(&mut platform_terminal, terminal_config);
	enable_terminal_features_with_config(&mut platform_terminal, terminal_config)?;
	let events = platform_terminal.event_reader();

	let backend = TerminaBackend::new(platform_terminal);
	let mut terminal = Terminal::new(backend)?;

	editor.ui_startup();
	let hook_runtime = editor.hook_runtime_mut();
	emit_hook_sync_with(&HookContext::new(HookEventData::EditorStart), hook_runtime);

	let mut last_cursor_style: Option<Cursor> = None;
	let mut dir = editor.pump().await;
	dir.needs_redraw = true;

	let result: io::Result<()> = async {
		loop {
			if dir.should_quit {
				break;
			}

			if dir.needs_redraw {
				terminal.draw(|frame| {
					#[cfg(feature = "perf")]
					let t0 = std::time::Instant::now();

					editor.render(frame);

					#[cfg(feature = "perf")]
					tracing::debug!(
						target: "perf",
						term_editor_render_ns = t0.elapsed().as_nanos() as u64,
					);
				})?;
			}

			let style = Cursor::CursorStyle(to_termina_cursor_style(dir.cursor_style));
			if last_cursor_style.as_ref() != Some(&style) {
				write!(terminal.backend_mut().terminal_mut(), "{}", Csi::Cursor(style))?;
				terminal.backend_mut().terminal_mut().flush()?;
				last_cursor_style = Some(style);
			}

			let mut filter = |e: &termina::event::Event| !e.is_escape();
			let has_event = match dir.poll_timeout {
				Some(t) => events.poll(Some(t), &mut filter)?,
				None => true,
			};

			if !has_event {
				dir = editor.pump().await;
				continue;
			}

			let mut event = events.read(&mut filter)?;
			if let termina::event::Event::WindowResized(size) = event {
				event = termina::event::Event::WindowResized(coalesce_resize_events(&events, size)?);
			}

			dir = editor.on_event(event).await;
		}
		Ok(())
	}
	.await;

	emit_hook(&HookContext::new(HookEventData::EditorQuit)).await;

	let terminal_inner = terminal.backend_mut().terminal_mut();
	let cleanup_result = disable_terminal_features_with_config(terminal_inner, terminal_config);

	result.and(cleanup_result)
}

fn to_termina_cursor_style(cs: CursorStyle) -> termina::style::CursorStyle {
	match cs {
		CursorStyle::Block => termina::style::CursorStyle::SteadyBlock,
		CursorStyle::Beam => termina::style::CursorStyle::SteadyBar,
		CursorStyle::Underline => termina::style::CursorStyle::SteadyUnderline,
		CursorStyle::Hidden => termina::style::CursorStyle::Default,
	}
}
