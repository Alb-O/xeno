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

pub fn run_editor(mut editor: Editor) -> io::Result<()> {
    let mut terminal = PlatformTerminal::new()?;
    install_panic_hook(&mut terminal);
    enable_terminal_features(&mut terminal)?;
    let events = terminal.event_reader();

    let backend = TerminaBackend::new(terminal);
    let mut terminal = Terminal::new(backend)?;

    // Pre-warm an embedded shell in the background so opening the terminal panel is instant.
    editor.start_terminal_prewarm();

    let result = (|| {
        loop {
            editor.poll_terminal_prewarm();

            let mut terminal_exited = false;
            if let Some(term) = &mut editor.terminal {
                term.update();
                if !term.is_alive() {
                    terminal_exited = true;
                }
            }
            if terminal_exited {
                editor.on_terminal_exit();
            }

            terminal.draw(|frame| editor.render(frame))?;

            // Set terminal cursor style based on mode.
            // When the embedded terminal is focused, prefer the user's terminal default.
            let cursor_style = if editor.terminal_focused {
                termina::style::CursorStyle::Default
            } else {
                cursor_style_for_mode(editor.mode())
            };
            write!(
                terminal.backend_mut().terminal_mut(),
                "{}",
                Csi::Cursor(Cursor::CursorStyle(cursor_style))
            )?;
            terminal.backend_mut().terminal_mut().flush()?;

            let mut filter = |e: &Event| !e.is_escape();
            let timeout = if matches!(editor.mode(), tome_core::Mode::Insert) || editor.terminal_open {
                Some(Duration::from_millis(16)) // ~60fps
            } else {
                None
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
