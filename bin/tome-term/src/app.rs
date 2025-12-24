use std::io::{self, Write};
use std::time::Duration;

use ratatui::Terminal;
use termina::escape::csi::{Csi, Cursor};
use termina::event::{Event, KeyEventKind};
use termina::{PlatformTerminal, Terminal as _};

use crate::backend::TerminaBackend;
use crate::editor::Editor;
use crate::terminal::{
	coalesce_resize_events, cursor_style_for_mode, disable_terminal_features,
	enable_terminal_features, install_panic_hook,
};

pub fn run_editor(
	mut editor: Editor,
	startup_ex: Option<String>,
	quit_after_ex: bool,
) -> io::Result<()> {
	let mut terminal = PlatformTerminal::new()?;
	install_panic_hook(&mut terminal);
	enable_terminal_features(&mut terminal)?;
	let events = terminal.event_reader();

	let backend = TerminaBackend::new(terminal);
	let mut terminal = Terminal::new(backend)?;

	// Start UI panels (includes terminal prewarm).
	editor.ui_startup();
	editor.autoload_plugins();

	if let Some(cmd) = startup_ex.as_deref() {
		let should_quit = editor.execute_ex_command(cmd);
		if quit_after_ex || should_quit {
			let terminal_inner = terminal.backend_mut().terminal_mut();
			let cleanup_result = disable_terminal_features(terminal_inner);
			return cleanup_result;
		}
	}

	let result = (|| {
		loop {
			editor.ui_tick();
			editor.poll_plugins();
			editor.poll_acp_events();

			terminal.draw(|frame| editor.render(frame))?;

			// Set terminal cursor style based on mode.
			// When a focused panel requests a cursor style, prefer it.
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
			let timeout = if matches!(editor.mode(), tome_core::Mode::Insert)
				|| editor.any_panel_open()
				|| editor.needs_redraw
			{
				editor.needs_redraw = false;
				Some(Duration::from_millis(16))
			} else {
				Some(Duration::from_millis(50))
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
					if editor.handle_key(key) {
						break;
					}
				}
				Event::Mouse(mouse) => {
					if editor.handle_mouse(mouse) {
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
	})();

	let terminal_inner = terminal.backend_mut().terminal_mut();
	let cleanup_result = disable_terminal_features(terminal_inner);

	result.and(cleanup_result)
}
