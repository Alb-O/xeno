#![cfg_attr(test, allow(unused_crate_dependencies))]

mod backend;
mod compositor;
mod document;
mod layer;
mod layers;
mod panels;
mod render_adapter;
mod scene;
mod terminal;
mod text_width;

use std::io::{self, Write};
use std::time::{Duration, Instant};

use termina::escape::csi::{Csi, Cursor};
use termina::{PlatformTerminal, Terminal as _};
use xeno_editor::runtime::{CursorStyle, RuntimeEvent};
use xeno_editor::{Editor, TerminalConfig};
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
	editor.emit_editor_start_hook();

	let mut last_cursor_style: Option<Cursor> = None;
	let mut notifications = crate::layers::notifications::FrontendNotifications::new();
	let mut last_notification_tick = Instant::now();
	let mut dir = editor.pump().await;
	dir.needs_redraw = true;

	let result: io::Result<()> = async {
		loop {
			if dir.should_quit {
				break;
			}

			let now = Instant::now();
			let notification_delta = now.saturating_duration_since(last_notification_tick);
			last_notification_tick = now;
			notifications.tick(notification_delta);
			if notifications.has_active_toasts() {
				dir.needs_redraw = true;
			}

			if dir.needs_redraw {
				terminal.draw(|frame| {
					#[cfg(feature = "perf")]
					let t0 = std::time::Instant::now();

					compositor::render_frame(&mut editor, frame, &mut notifications);

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
			let poll_timeout = if notifications.has_active_toasts() {
				Some(Duration::from_millis(16))
			} else {
				dir.poll_timeout
			};

			let has_event = match poll_timeout {
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

			if let Some(event) = map_terminal_event(event) {
				dir = editor.on_event(event).await;
			} else {
				dir = editor.pump().await;
			}
		}
		Ok(())
	}
	.await;

	editor.emit_editor_quit_hook().await;

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

fn map_terminal_event(event: termina::event::Event) -> Option<RuntimeEvent> {
	use termina::event::{Event, KeyEventKind};

	match event {
		Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => Some(RuntimeEvent::Key(key.into())),
		Event::Mouse(mouse) => Some(RuntimeEvent::Mouse(mouse.into())),
		Event::Paste(content) => Some(RuntimeEvent::Paste(content)),
		Event::WindowResized(size) => Some(RuntimeEvent::WindowResized {
			cols: size.cols,
			rows: size.rows,
		}),
		Event::FocusIn => Some(RuntimeEvent::FocusIn),
		Event::FocusOut => Some(RuntimeEvent::FocusOut),
		_ => None,
	}
}
