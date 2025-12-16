mod backend;
mod capabilities;
mod cli;
mod editor;
mod render;
mod styles;
pub mod theme;
pub mod themes;
#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::time::Duration;

use clap::Parser;
use ratatui::Terminal;
use termina::{EventReader, PlatformTerminal, Terminal as _, WindowSize};
use termina::event::{Event, KeyEventKind};
use termina::escape::csi::{Csi, Cursor, Keyboard, KittyKeyboardFlags, Mode, DecPrivateMode, DecPrivateModeCode};
use termina::style::CursorStyle;

use cli::Cli;
use backend::TerminaBackend;
pub use editor::Editor;

fn enable_terminal_features(terminal: &mut PlatformTerminal) -> io::Result<()> {
    terminal.enter_raw_mode()?;
    write!(
        terminal,
        "{}{}",
        Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(
            DecPrivateModeCode::ClearAndEnableAlternateScreen
        ))),
        Csi::Keyboard(Keyboard::PushFlags(
            KittyKeyboardFlags::DISAMBIGUATE_ESCAPE_CODES
        ))
    )?;
    write!(
        terminal,
        "{}{}{}",
        Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::MouseTracking))),
        Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse))),
        Csi::Mode(Mode::SetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse))),
    )?;
    terminal.flush()
}

fn disable_terminal_features(terminal: &mut PlatformTerminal) -> io::Result<()> {
    write!(
        terminal,
        "{}{}{}{}{}{}",
        Csi::Cursor(Cursor::CursorStyle(CursorStyle::Default)),
        Csi::Keyboard(Keyboard::PopFlags(1)),
        Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::MouseTracking))),
        Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse))),
        Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse))),
        Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ClearAndEnableAlternateScreen)))
    )?;
    terminal.enter_cooked_mode()?;
    terminal.flush()
}

fn install_panic_hook(terminal: &mut PlatformTerminal) {
    terminal.set_panic_hook(|handle| {
        let _ = write!(
            handle,
            "{}{}{}{}{}{}",
            Csi::Cursor(Cursor::CursorStyle(CursorStyle::Default)),
            Csi::Keyboard(Keyboard::PopFlags(1)),
            Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::MouseTracking))),
            Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::SGRMouse))),
            Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::AnyEventMouse))),
            Csi::Mode(Mode::ResetDecPrivateMode(DecPrivateMode::Code(DecPrivateModeCode::ClearAndEnableAlternateScreen)))
        );
        let _ = handle.flush();
    });
}

fn coalesce_resize_events(events: &EventReader, first: WindowSize) -> io::Result<WindowSize> {
    let mut filter = |event: &Event| matches!(event, Event::WindowResized(_));
    let mut latest = first;

    while events.poll(Some(Duration::from_millis(0)), &mut filter)? {
        if let Event::WindowResized(size) = events.read(&mut filter)? {
            latest = size;
        }
    }

    Ok(latest)
}

fn cursor_style_for_mode(mode: tome_core::Mode) -> CursorStyle {
    match mode {
        tome_core::Mode::Insert => CursorStyle::BlinkingBar,
        _ => CursorStyle::SteadyBlock,
    }
}

fn run_editor(mut editor: Editor) -> io::Result<()> {
    let mut terminal = PlatformTerminal::new()?;
    install_panic_hook(&mut terminal);
    enable_terminal_features(&mut terminal)?;
    let events = terminal.event_reader();

    let backend = TerminaBackend::new(terminal);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        loop {
            terminal.draw(|frame| editor.render(frame))?;

            // Set terminal cursor style based on mode
            let cursor_style = cursor_style_for_mode(editor.mode());
            write!(
                terminal.backend_mut().terminal_mut(),
                "{}",
                Csi::Cursor(Cursor::CursorStyle(cursor_style))
            )?;
            terminal.backend_mut().terminal_mut().flush()?;

            let event = events.read(|e| !e.is_escape())?;

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

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let editor = match cli.file {
        Some(path) => Editor::new(path)?,
        None => Editor::new_scratch(),
    };

    run_editor(editor)
}
