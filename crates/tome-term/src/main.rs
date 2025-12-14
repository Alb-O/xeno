use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use tome_core::range::Direction as MoveDir;
use tome_core::{Rope, Selection, Transaction, movement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Insert,
}

impl Mode {
    fn name(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
        }
    }
}

struct Editor {
    doc: Rope,
    selection: Selection,
    mode: Mode,
    path: PathBuf,
    modified: bool,
    scroll_offset: usize,
    message: Option<String>,
}

impl Editor {
    fn new(path: PathBuf) -> io::Result<Self> {
        let content = if path.exists() {
            fs::read_to_string(&path)?
        } else {
            String::new()
        };

        Ok(Self::from_content(content, path))
    }

    fn from_content(content: String, path: PathBuf) -> Self {
        Self {
            doc: Rope::from(content.as_str()),
            selection: Selection::point(0),
            mode: Mode::Normal,
            path,
            modified: false,
            scroll_offset: 0,
            message: None,
        }
    }

    fn cursor_line(&self) -> usize {
        let head = self.selection.primary().head;
        self.doc
            .char_to_line(head.min(self.doc.len_chars().saturating_sub(1).max(0)))
    }

    fn cursor_col(&self) -> usize {
        let head = self.selection.primary().head;
        let line = self.cursor_line();
        let line_start = self.doc.line_to_char(line);
        head.saturating_sub(line_start)
    }

    fn adjust_scroll(&mut self, viewport_height: usize) {
        let cursor_line = self.cursor_line();
        if cursor_line < self.scroll_offset {
            self.scroll_offset = cursor_line;
        } else if cursor_line >= self.scroll_offset + viewport_height {
            self.scroll_offset = cursor_line.saturating_sub(viewport_height - 1);
        }
    }

    fn insert_text(&mut self, text: &str) {
        let tx = Transaction::insert(self.doc.slice(..), &self.selection, text.to_string());
        tx.apply(&mut self.doc);
        // After insert, move cursor to end of inserted text (collapse selection to point)
        let head = self.selection.primary().head + text.chars().count();
        self.selection = Selection::point(head);
        self.modified = true;
    }

    fn save(&mut self) -> io::Result<()> {
        let mut f = fs::File::create(&self.path)?;
        for chunk in self.doc.chunks() {
            f.write_all(chunk.as_bytes())?;
        }
        self.modified = false;
        self.message = Some(format!("Saved {}", self.path.display()));
        Ok(())
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> bool {
        let slice = self.doc.slice(..);
        match key.code {
            KeyCode::Char('q') => return true,

            KeyCode::Char('h') | KeyCode::Left => {
                let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                self.selection.transform_mut(|r| {
                    *r = movement::move_horizontally(slice, *r, MoveDir::Backward, 1, extend);
                });
            }

            KeyCode::Char('l') | KeyCode::Right => {
                let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                self.selection.transform_mut(|r| {
                    *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, extend);
                });
            }

            KeyCode::Char('j') | KeyCode::Down => {
                let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(slice, *r, MoveDir::Forward, 1, extend);
                });
            }

            KeyCode::Char('k') | KeyCode::Up => {
                let extend = key.modifiers.contains(KeyModifiers::SHIFT);
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(slice, *r, MoveDir::Backward, 1, extend);
                });
            }

            KeyCode::Char('0') | KeyCode::Home => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_start(slice, *r, false);
                });
            }

            KeyCode::Char('$') | KeyCode::End => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_end(slice, *r, false);
                });
            }

            KeyCode::Char('^') => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_first_nonwhitespace(slice, *r, false);
                });
            }

            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
            }

            KeyCode::Char('a') => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, false);
                });
                self.mode = Mode::Insert;
            }

            KeyCode::Char('I') => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_first_nonwhitespace(slice, *r, false);
                });
                self.mode = Mode::Insert;
            }

            KeyCode::Char('A') => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_end(slice, *r, false);
                });
                self.mode = Mode::Insert;
            }

            KeyCode::Char('o') => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_end(slice, *r, false);
                });
                self.insert_text("\n");
                self.mode = Mode::Insert;
            }

            KeyCode::Char('O') => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_start(slice, *r, false);
                });
                self.insert_text("\n");
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(
                        self.doc.slice(..),
                        *r,
                        MoveDir::Backward,
                        1,
                        false,
                    );
                });
                self.mode = Mode::Insert;
            }

            KeyCode::Char('x') => {
                if self.selection.primary().is_empty() {
                    self.selection.transform_mut(|r| {
                        *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, true);
                    });
                }
                if !self.selection.primary().is_empty() {
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    self.modified = true;
                }
            }

            KeyCode::Char('d') => {
                if !self.selection.primary().is_empty() {
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    self.modified = true;
                }
            }

            KeyCode::Char(';') => {
                self.selection.transform_mut(|r| {
                    r.anchor = r.head;
                });
            }

            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Err(e) = self.save() {
                    self.message = Some(format!("Error saving: {}", e));
                }
            }

            KeyCode::Char('g') => {
                self.selection = Selection::point(0);
            }

            KeyCode::Char('G') => {
                let last = self.doc.len_chars().saturating_sub(1);
                self.selection = Selection::point(last.max(0));
            }

            _ => {}
        }
        false
    }

    fn handle_insert_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }

            KeyCode::Char(c) => {
                self.insert_text(&c.to_string());
            }

            KeyCode::Enter => {
                self.insert_text("\n");
            }

            KeyCode::Tab => {
                self.insert_text("    ");
            }

            KeyCode::Backspace => {
                let slice = self.doc.slice(..);
                if self.selection.primary().is_empty() && self.selection.primary().head > 0 {
                    self.selection.transform_mut(|r| {
                        *r = movement::move_horizontally(slice, *r, MoveDir::Backward, 1, true);
                    });
                }
                if !self.selection.primary().is_empty() {
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    self.modified = true;
                }
            }

            KeyCode::Delete => {
                let slice = self.doc.slice(..);
                if self.selection.primary().is_empty() {
                    self.selection.transform_mut(|r| {
                        *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, true);
                    });
                }
                if !self.selection.primary().is_empty() {
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    self.modified = true;
                }
            }

            KeyCode::Left => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_horizontally(
                        self.doc.slice(..),
                        *r,
                        MoveDir::Backward,
                        1,
                        false,
                    );
                });
            }

            KeyCode::Right => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_horizontally(
                        self.doc.slice(..),
                        *r,
                        MoveDir::Forward,
                        1,
                        false,
                    );
                });
            }

            KeyCode::Up => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(
                        self.doc.slice(..),
                        *r,
                        MoveDir::Backward,
                        1,
                        false,
                    );
                });
            }

            KeyCode::Down => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(
                        self.doc.slice(..),
                        *r,
                        MoveDir::Forward,
                        1,
                        false,
                    );
                });
            }

            _ => {}
        }
        false
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.message = None;
        match self.mode {
            Mode::Normal => self.handle_normal_key(key),
            Mode::Insert => self.handle_insert_key(key),
        }
    }

    fn render_document(&self, area: Rect) -> impl Widget + '_ {
        let start_line = self.scroll_offset;
        let end_line = (start_line + area.height as usize).min(self.doc.len_lines());

        let primary = self.selection.primary();
        let sel_start = primary.from();
        let sel_end = primary.to();

        let mut lines = Vec::with_capacity(end_line - start_line);

        for line_idx in start_line..end_line {
            let line_start = self.doc.line_to_char(line_idx);
            let line_end = if line_idx + 1 < self.doc.len_lines() {
                self.doc.line_to_char(line_idx + 1)
            } else {
                self.doc.len_chars()
            };

            let line_text: String = self.doc.slice(line_start..line_end).into();
            let line_text = line_text.trim_end_matches('\n');

            let mut spans = Vec::new();

            for (char_idx, ch) in line_text.chars().enumerate() {
                let doc_pos = line_start + char_idx;
                let in_selection = doc_pos >= sel_start && doc_pos < sel_end;
                let is_cursor = doc_pos == primary.head;

                let style = if is_cursor {
                    Style::default()
                        .bg(Color::White)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                } else if in_selection {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };

                spans.push(Span::styled(ch.to_string(), style));
            }

            if primary.head == line_end.saturating_sub(1) && line_text.is_empty() {
                spans.push(Span::styled(
                    " ",
                    Style::default()
                        .bg(Color::White)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ));
            } else if primary.head == line_end && line_idx + 1 >= self.doc.len_lines() {
                spans.push(Span::styled(
                    " ",
                    Style::default()
                        .bg(Color::White)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            lines.push(Line::from(spans));
        }

        Paragraph::new(lines)
    }

    fn render_status_line(&self) -> impl Widget + '_ {
        let mode_style = match self.mode {
            Mode::Normal => Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            Mode::Insert => Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        };

        let modified = if self.modified { " [+]" } else { "" };
        let path = self.path.display().to_string();
        let cursor_info = format!(" {}:{} ", self.cursor_line() + 1, self.cursor_col() + 1);

        let spans = vec![
            Span::styled(format!(" {} ", self.mode.name()), mode_style),
            Span::styled(
                format!(" {}{} ", path, modified),
                Style::default().add_modifier(Modifier::REVERSED),
            ),
            Span::styled(
                cursor_info,
                Style::default().add_modifier(Modifier::REVERSED),
            ),
        ];

        Paragraph::new(Line::from(spans))
    }

    fn render_message_line(&self) -> impl Widget + '_ {
        let text = self.message.as_deref().unwrap_or("");
        Paragraph::new(text).style(Style::default().fg(Color::Yellow))
    }

    fn render(&self, frame: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(frame.area());

        frame.render_widget(self.render_document(chunks[0]), chunks[0]);
        frame.render_widget(self.render_status_line(), chunks[1]);
        frame.render_widget(self.render_message_line(), chunks[2]);
    }
}

fn run_editor(mut editor: Editor) -> io::Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        loop {
            let viewport_height = terminal.size()?.height.saturating_sub(2) as usize;
            editor.adjust_scroll(viewport_height);

            terminal.draw(|frame| editor.render(frame))?;

            if let Event::Key(key) = crossterm::event::read()? {
                if editor.handle_key(key) {
                    break;
                }
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
    terminal.show_cursor()?;

    result
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: tome <file>");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let editor = Editor::new(path)?;
    run_editor(editor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    fn test_editor(content: &str) -> Editor {
        Editor::from_content(content.to_string(), PathBuf::from("test.txt"))
    }

    #[test]
    fn test_render_empty() {
        let editor = test_editor("");
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_with_content() {
        let editor = test_editor("Hello, World!\nThis is a test.\nLine 3.");
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_insert_mode() {
        let mut editor = test_editor("Hello");
        editor.mode = Mode::Insert;
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_after_typing() {
        let mut editor = test_editor("");
        editor.mode = Mode::Insert;
        editor.insert_text("abc");
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_with_selection() {
        let mut editor = test_editor("Hello, World!");
        editor.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::SHIFT));
        editor.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::SHIFT));
        editor.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::SHIFT));
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
}
