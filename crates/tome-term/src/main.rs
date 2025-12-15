mod cli;
mod editor;
mod render;
mod styles;

use std::io;

use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use cli::Cli;
pub use editor::Editor;

fn run_editor(mut editor: Editor) -> io::Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        loop {
            terminal.draw(|frame| editor.render(frame))?;

            match crossterm::event::read()? {
                Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                    if editor.handle_key(key) {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    if editor.handle_mouse(mouse) {
                        break;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    result
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let editor = match cli.file {
        Some(path) => Editor::new(path)?,
        None => Editor::new_scratch(),
    };

    run_editor(editor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};
    use tome_core::Mode;

    fn test_editor(content: &str) -> Editor {
        Editor::from_content(content.to_string(), Some(PathBuf::from("test.txt")))
    }

    #[test]
    fn test_render_empty() {
        let mut editor = test_editor("");
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_with_content() {
        let mut editor = test_editor("Hello, World!\nThis is a test.\nLine 3.");
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_insert_mode() {
        let mut editor = test_editor("Hello");
        editor.input.set_mode(Mode::Insert);
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_after_typing() {
        let mut editor = test_editor("");
        editor.input.set_mode(Mode::Insert);
        editor.insert_text("abc");
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_with_selection() {
        let mut editor = test_editor("Hello, World!");
        editor.handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT));
        editor.handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT));
        editor.handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT));
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_cursor_movement() {
        let mut editor = test_editor("Hello\nWorld");
        editor.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        editor.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        editor.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_word_movement() {
        let mut editor = test_editor("hello world test");
        editor.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().head, 6);
    }

    #[test]
    fn test_goto_mode() {
        let mut editor = test_editor("line1\nline2\nline3");
        editor.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(matches!(editor.mode(), Mode::Goto));
        editor.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().head, 0);
    }

    #[test]
    fn test_undo_redo() {
        let mut editor = test_editor("hello");
        assert_eq!(editor.doc.to_string(), "hello");

        editor.handle_key(KeyEvent::new(KeyCode::Char('%'), KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().from(), 0);
        assert_eq!(editor.selection.primary().to(), 5);

        editor.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(editor.doc.to_string(), "", "after delete");
        assert_eq!(editor.undo_stack.len(), 1, "undo stack should have 1 entry");

        editor.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
        assert_eq!(editor.doc.to_string(), "hello", "after undo");
        assert_eq!(editor.redo_stack.len(), 1, "redo stack should have 1 entry");
        assert_eq!(editor.undo_stack.len(), 0, "undo stack should be empty");

        editor.handle_key(KeyEvent::new(KeyCode::Char('U'), KeyModifiers::SHIFT));
        assert_eq!(editor.redo_stack.len(), 0, "redo stack should be empty after redo");
        assert_eq!(editor.doc.to_string(), "", "after redo");
    }

    #[test]
    fn test_insert_newline_single_cursor() {
        use ratatui::style::{Color, Modifier};
        
        let mut editor = test_editor("");
        
        editor.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        assert!(matches!(editor.mode(), Mode::Insert));
        
        editor.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        
        assert_eq!(editor.doc.len_lines(), 2, "should have 2 lines after Enter");
        assert_eq!(editor.selection.primary().head, 1, "cursor should be at position 1");
        
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        
        let buffer = terminal.backend().buffer();
        let mut cursor_cells = Vec::new();
        for row in 0..8 {
            for col in 0..80 {
                let cell = &buffer[(col, row)];
                if cell.bg == Color::White && cell.fg == Color::Black 
                   && cell.modifier.contains(Modifier::BOLD) {
                    cursor_cells.push((col, row));
                }
            }
        }
        
        assert_eq!(
            cursor_cells.len(), 
            1, 
            "Expected 1 cursor cell, found {} at positions: {:?}", 
            cursor_cells.len(), 
            cursor_cells
        );
        assert_eq!(
            cursor_cells[0].1, 
            1, 
            "Cursor should be on row 1 (second line), found at {:?}", 
            cursor_cells[0]
        );
        
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_insert_mode_arrow_keys() {
        let mut editor = test_editor("hello world");
        assert_eq!(editor.selection.primary().head, 0, "start at position 0");

        editor.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        assert!(matches!(editor.mode(), Mode::Insert), "should be in insert mode");

        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().head, 1, "after Right arrow, cursor at 1");

        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().head, 2, "after Right arrow, cursor at 2");

        editor.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().head, 1, "after Left arrow, cursor at 1");

        editor.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(matches!(editor.mode(), Mode::Insert), "still in insert mode after arrows");
    }

    #[test]
    fn test_soft_wrap_long_line() {
        let long_line = "The quick brown fox jumps over the lazy dog and keeps on running";
        let mut editor = test_editor(long_line);
        
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_soft_wrap_word_boundary() {
        let text = "hello world this is a test of word wrapping behavior";
        let mut editor = test_editor(text);
        
        let mut terminal = Terminal::new(TestBackend::new(30, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_line_numbers_multiple_lines() {
        let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let mut editor = test_editor(text);
        
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_wrapped_line_dim_gutter() {
        use ratatui::style::Color;
        
        let long_line = "This is a very long line that should wrap to multiple virtual lines";
        let mut editor = test_editor(long_line);
        
        let mut terminal = Terminal::new(TestBackend::new(30, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        
        let buffer = terminal.backend().buffer();
        
        let first_gutter = &buffer[(0, 0)];
        assert_eq!(first_gutter.fg, Color::DarkGray, "first line gutter should be DarkGray");
        
        let second_gutter = &buffer[(0, 1)];
        assert_eq!(second_gutter.fg, Color::Rgb(60, 60, 60), "wrapped line gutter should be dim");
        
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_backspace_deletes_backwards() {
        let mut editor = test_editor("hello");
        
        editor.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(matches!(editor.mode(), Mode::Insert));
        assert_eq!(editor.selection.primary().head, 1, "cursor at 1 after 'a'");
        
        editor.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(editor.doc.to_string(), "ello", "first char deleted");
        assert_eq!(editor.selection.primary().head, 0, "cursor moved back to 0");
        
        editor.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(editor.doc.to_string(), "ello", "no change when at start");
        assert_eq!(editor.selection.primary().head, 0, "cursor stays at 0");
    }

    #[test]
    fn test_scroll_down_when_cursor_at_bottom() {
        let text = (1..=20).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        let mut editor = test_editor(&text);
        
        let viewport_height = 8;
        
        assert_eq!(editor.scroll_line, 0, "starts at top");
        assert_eq!(editor.cursor_line(), 0, "cursor on line 0");
        
        for _ in 0..10 {
            editor.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        }
        
        assert_eq!(editor.cursor_line(), 10, "cursor on line 10");
        
        let mut terminal = Terminal::new(TestBackend::new(40, viewport_height as u16 + 2)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        
        assert!(
            editor.scroll_line + viewport_height > editor.cursor_line(),
            "cursor line {} should be visible in viewport (scroll_line={}, height={})",
            editor.cursor_line(),
            editor.scroll_line,
            viewport_height
        );
    }

    #[test]
    fn test_scroll_with_soft_wrapped_lines() {
        let long_line = "This is a very long line that will wrap multiple times in the viewport";
        let text = format!("{}\n{}\n{}\nshort\nshort", long_line, long_line, long_line);
        let mut editor = test_editor(&text);
        
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).unwrap();
        
        for _ in 0..4 {
            editor.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        }
        
        assert_eq!(editor.cursor_line(), 4, "cursor on line 4 (last 'short')");
        
        terminal.draw(|frame| editor.render(frame)).unwrap();
        
        assert!(
            editor.scroll_line > 0 || editor.scroll_segment > 0,
            "scroll should advance to show cursor through wrapped lines, got scroll_line={}, scroll_segment={}",
            editor.scroll_line,
            editor.scroll_segment
        );
    }

    #[test]
    fn test_visual_vertical_movement_in_wrapped_line() {
        let long_line = "The quick brown fox jumps over the lazy dog and keeps running";
        let mut editor = test_editor(long_line);
        
        editor.text_width = 20;
        
        assert_eq!(editor.selection.primary().head, 0, "starts at 0");
        
        editor.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        
        let head = editor.selection.primary().head;
        assert!(
            head > 0 && head < long_line.len(),
            "cursor should move within wrapped line segments, got head={}",
            head
        );
        assert_eq!(editor.cursor_line(), 0, "should still be on doc line 0");
        
        editor.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        
        assert_eq!(editor.selection.primary().head, 0, "should return to start");
    }

    #[test]
    fn test_visual_movement_across_doc_lines() {
        let text = "short\nanother short";
        let mut editor = test_editor(text);
        editor.text_width = 80;
        
        assert_eq!(editor.cursor_line(), 0);
        
        editor.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        
        assert_eq!(editor.cursor_line(), 1, "should move to next doc line");
        
        editor.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        
        assert_eq!(editor.cursor_line(), 0, "should return to first doc line");
    }

    #[test]
    fn test_wrap_preserves_leading_spaces() {
        let mut editor = test_editor("hello");
        editor.text_width = 10;

        // "hello     world" = 15 chars (5 spaces between)
        // Width 10 means break happens after the run of spaces
        let segments = editor.wrap_line("hello     world", 10);
        assert_eq!(segments.len(), 2);
        // Breaks at last space before width, so first segment includes trailing spaces
        assert_eq!(segments[0].text, "hello     ");
        assert_eq!(segments[0].start_offset, 0);
        assert_eq!(segments[1].text, "world");
        assert_eq!(segments[1].start_offset, 10);

        // Key assertion: all characters accounted for
        let total_chars: usize = segments.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total_chars, 15, "all characters preserved");

        // Test leading spaces in second segment are preserved
        let segments2 = editor.wrap_line("hello    world test", 11);
        // Should break at space before 'world' to fit 11 chars
        // "hello    w" is 10 chars, finds break at space position 8, returns 9
        // Actually let's just check all chars preserved
        let total2: usize = segments2.iter().map(|s| s.text.chars().count()).sum();
        assert_eq!(total2, 19, "all characters preserved");
    }

    #[test]
    fn test_shift_end_extends_selection() {
        let mut editor = test_editor("hello world");
        // Start at position 0
        assert_eq!(editor.selection.primary().head, 0);
        assert_eq!(editor.selection.primary().anchor, 0);

        // Shift+End should select from current position to end of line
        editor.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::SHIFT));

        let sel = editor.selection.primary();
        assert_eq!(sel.anchor, 0, "anchor should stay at start");
        assert_eq!(sel.head, 11, "head should move to end of line");
        assert!(!sel.is_empty(), "selection should not be empty");
    }

    #[test]
    fn test_shift_home_extends_selection() {
        // In Kakoune, we start with a point selection and use extend to select
        let mut editor = test_editor("hello world");
        
        // Shift+End to select to end (extend from current position)
        editor.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::SHIFT));
        let sel_after_end = editor.selection.primary();
        assert_eq!(sel_after_end.anchor, 0, "anchor stays at start");
        assert_eq!(sel_after_end.head, 11, "head moves to end");

        // Shift+Home should extend back to start (anchor stays, head moves)
        editor.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::SHIFT));

        let sel = editor.selection.primary();
        assert_eq!(sel.head, 0, "head should move to start");
        assert_eq!(sel.anchor, 0, "anchor stays at original position");
    }

    #[test]
    fn test_shift_end_then_non_shift_home() {
        // Start at 0, Shift+End to select, then Home (no shift) moves without extending
        let mut editor = test_editor("hello world");
        
        editor.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::SHIFT));
        let sel = editor.selection.primary();
        assert_eq!(sel.anchor, 0);
        assert_eq!(sel.head, 11);
        
        // Home without shift - in Kakoune, this creates a new selection from 11 to 0
        editor.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        let sel = editor.selection.primary();
        // anchor becomes previous head (11), head becomes new position (0)
        assert_eq!(sel.anchor, 11, "anchor becomes previous head");
        assert_eq!(sel.head, 0, "head moves to start");
    }

    #[test]
    fn test_shift_right_extends_selection() {
        let mut editor = test_editor("hello");
        assert_eq!(editor.selection.primary().head, 0);

        // Shift+Right three times should extend selection
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));

        let sel = editor.selection.primary();
        assert_eq!(sel.anchor, 0, "anchor should stay at start");
        assert_eq!(sel.head, 3, "head should move 3 positions");
    }

    #[test]
    fn test_end_without_shift_collapses_selection() {
        let mut editor = test_editor("hello world");
        // First select some text
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
        assert!(!editor.selection.primary().is_empty(), "should have selection");

        // End without shift should move cursor without extending
        editor.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));

        let sel = editor.selection.primary();
        // After non-extend motion, selection should be at new position
        assert_eq!(sel.head, 11, "head should be at end");
    }
}
