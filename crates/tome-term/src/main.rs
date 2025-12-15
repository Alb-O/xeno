use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
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
use tome_core::{
    Command, InputHandler, Key, KeyResult, Mode, ObjectType, Rope, Selection, Transaction, WordType, movement,
};
use tome_core::keymap::ObjectSelection;

/// A history entry for undo/redo.
#[derive(Clone)]
struct HistoryEntry {
    /// The document state before the transaction.
    doc: Rope,
    /// The selection before the transaction.
    selection: Selection,
}

/// A segment of a wrapped line.
struct WrapSegment {
    /// The text content of this segment.
    text: String,
    /// Character offset from the start of the original line.
    start_offset: usize,
}

struct Editor {
    doc: Rope,
    selection: Selection,
    input: InputHandler,
    path: PathBuf,
    modified: bool,
    scroll_offset: usize,
    message: Option<String>,
    registers: Registers,
    /// Undo stack.
    undo_stack: Vec<HistoryEntry>,
    /// Redo stack.
    redo_stack: Vec<HistoryEntry>,
}

#[derive(Default)]
struct Registers {
    yank: String,
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
            input: InputHandler::new(),
            path,
            modified: false,
            scroll_offset: 0,
            message: None,
            registers: Registers::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    fn mode(&self) -> Mode {
        self.input.mode()
    }

    fn mode_name(&self) -> &'static str {
        self.input.mode_name()
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

    /// Save state for undo before making changes.
    fn save_undo_state(&mut self) {
        self.undo_stack.push(HistoryEntry {
            doc: self.doc.clone(),
            selection: self.selection.clone(),
        });
        // Clear redo stack when making new changes
        self.redo_stack.clear();

        // Limit undo stack size
        const MAX_UNDO: usize = 100;
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
    }

    /// Undo the last change.
    fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            // Save current state for redo
            self.redo_stack.push(HistoryEntry {
                doc: self.doc.clone(),
                selection: self.selection.clone(),
            });

            self.doc = entry.doc;
            self.selection = entry.selection;
            self.message = Some("Undo".to_string());
        } else {
            self.message = Some("Nothing to undo".to_string());
        }
    }

    /// Redo the last undone change.
    fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            // Save current state for undo
            self.undo_stack.push(HistoryEntry {
                doc: self.doc.clone(),
                selection: self.selection.clone(),
            });

            self.doc = entry.doc;
            self.selection = entry.selection;
            self.message = Some("Redo".to_string());
        } else {
            self.message = Some("Nothing to redo".to_string());
        }
    }

    fn insert_text(&mut self, text: &str) {
        self.save_undo_state();
        let tx = Transaction::insert(self.doc.slice(..), &self.selection, text.to_string());
        tx.apply(&mut self.doc);
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

    fn yank_selection(&mut self) {
        let primary = self.selection.primary();
        let from = primary.from();
        let to = primary.to();
        if from < to {
            self.registers.yank = self.doc.slice(from..to).to_string();
            self.message = Some(format!("Yanked {} chars", to - from));
        }
    }

    fn paste_after(&mut self) {
        if self.registers.yank.is_empty() {
            return;
        }
        let slice = self.doc.slice(..);
        self.selection.transform_mut(|r| {
            *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, false);
        });
        self.insert_text(&self.registers.yank.clone());
    }

    fn paste_before(&mut self) {
        if self.registers.yank.is_empty() {
            return;
        }
        self.insert_text(&self.registers.yank.clone());
    }

    fn select_object(&mut self, obj: ObjectType, inner: bool) {
        let slice = self.doc.slice(..);
        self.selection.transform_mut(|r| {
            match obj {
                ObjectType::Word => {
                    *r = movement::select_word_object(slice, *r, WordType::Word, inner);
                }
                ObjectType::WORD => {
                    *r = movement::select_word_object(slice, *r, WordType::WORD, inner);
                }
                ObjectType::Parentheses
                | ObjectType::Braces
                | ObjectType::Brackets
                | ObjectType::AngleBrackets
                | ObjectType::DoubleQuotes
                | ObjectType::SingleQuotes
                | ObjectType::Backticks
                | ObjectType::Custom(_) => {
                    if let Some((open, close)) = obj.delimiters() {
                        if let Some(new_range) =
                            movement::select_surround_object(slice, *r, open, close, inner)
                        {
                            *r = new_range;
                        }
                    }
                }
                _ => {
                    // Other object types not yet implemented
                }
            }
        });
    }

    fn select_to_object_boundary(&mut self, obj: ObjectType, to_start: bool, extend: bool) {
        let slice = self.doc.slice(..);
        self.selection.transform_mut(|r| {
            // Get the full object range first
            let obj_range = match obj {
                ObjectType::Word => {
                    Some(movement::select_word_object(slice, *r, WordType::Word, false))
                }
                ObjectType::WORD => {
                    Some(movement::select_word_object(slice, *r, WordType::WORD, false))
                }
                ObjectType::Parentheses
                | ObjectType::Braces
                | ObjectType::Brackets
                | ObjectType::AngleBrackets
                | ObjectType::DoubleQuotes
                | ObjectType::SingleQuotes
                | ObjectType::Backticks
                | ObjectType::Custom(_) => {
                    if let Some((open, close)) = obj.delimiters() {
                        movement::select_surround_object(slice, *r, open, close, false)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(obj_range) = obj_range {
                let new_head = if to_start { obj_range.from() } else { obj_range.to() };
                if extend {
                    *r = tome_core::Range::new(r.anchor, new_head);
                } else {
                    *r = tome_core::Range::new(r.head, new_head);
                }
            }
        });
    }

    fn execute_command_line(&mut self, cmd: &str) -> bool {
        let cmd = cmd.trim();
        match cmd {
            "q" | "quit" => return true,
            "q!" | "quit!" => return true,
            "w" | "write" => {
                match self.save() {
                    Ok(()) => {}
                    Err(e) => self.message = Some(format!("Error saving: {}", e)),
                }
            }
            "wq" | "x" => {
                match self.save() {
                    Ok(()) => return true,
                    Err(e) => self.message = Some(format!("Error saving: {}", e)),
                }
            }
            _ => {
                self.message = Some(format!("Unknown command: {}", cmd));
            }
        }
        false
    }

    fn execute_command(&mut self, cmd: Command, count: u32, extend: bool) -> bool {
        let slice = self.doc.slice(..);
        // count of 0 means "no count specified", treat as 1
        let count_usize = if count == 0 { 1 } else { count as usize };

        match cmd {
            Command::MoveLeft => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_horizontally(slice, *r, MoveDir::Backward, count_usize, extend);
                });
            }
            Command::MoveRight => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_horizontally(slice, *r, MoveDir::Forward, count_usize, extend);
                });
            }
            Command::MoveUp => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(slice, *r, MoveDir::Backward, count_usize, extend);
                });
            }
            Command::MoveDown => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(slice, *r, MoveDir::Forward, count_usize, extend);
                });
            }

            Command::MoveNextWordStart => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_next_word_start(slice, *r, count_usize, WordType::Word, extend);
                });
            }
            Command::MovePrevWordStart => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_prev_word_start(slice, *r, count_usize, WordType::Word, extend);
                });
            }
            Command::MoveNextWordEnd => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_next_word_end(slice, *r, count_usize, WordType::Word, extend);
                });
            }
            Command::MoveNextWORDStart => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_next_word_start(slice, *r, count_usize, WordType::WORD, extend);
                });
            }
            Command::MovePrevWORDStart => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_prev_word_start(slice, *r, count_usize, WordType::WORD, extend);
                });
            }
            Command::MoveNextWORDEnd => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_next_word_end(slice, *r, count_usize, WordType::WORD, extend);
                });
            }

            Command::MoveLineStart => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_start(slice, *r, extend);
                });
            }
            Command::MoveLineEnd => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_end(slice, *r, extend);
                });
            }
            Command::MoveFirstNonWhitespace => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_first_nonwhitespace(slice, *r, extend);
                });
            }

            Command::MoveDocumentStart => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_document_start(slice, *r, extend);
                });
            }
            Command::MoveDocumentEnd => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_document_end(slice, *r, extend);
                });
            }

            Command::CollapseSelection => {
                self.selection.transform_mut(|r| {
                    r.anchor = r.head;
                });
            }
            Command::FlipSelection => {
                self.selection.transform_mut(|r| {
                    std::mem::swap(&mut r.anchor, &mut r.head);
                });
            }
            Command::EnsureForward => {
                self.selection.transform_mut(|r| {
                    if r.head < r.anchor {
                        std::mem::swap(&mut r.anchor, &mut r.head);
                    }
                });
            }

            Command::SelectLine => {
                self.selection.transform_mut(|r| {
                    let line = slice.char_to_line(r.head);
                    let start = slice.line_to_char(line);
                    let end = if line + 1 < slice.len_lines() {
                        slice.line_to_char(line + 1)
                    } else {
                        slice.len_chars()
                    };
                    r.anchor = start;
                    r.head = end;
                });
            }
            Command::SelectAll => {
                self.selection = Selection::single(0, self.doc.len_chars());
            }

            Command::Delete { yank } => {
                if yank {
                    self.yank_selection();
                }
                if self.selection.primary().is_empty() {
                    let slice = self.doc.slice(..);
                    self.selection.transform_mut(|r| {
                        *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, true);
                    });
                }
                if !self.selection.primary().is_empty() {
                    self.save_undo_state();
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    self.modified = true;
                }
            }
            Command::Change { yank } => {
                if yank {
                    self.yank_selection();
                }
                if !self.selection.primary().is_empty() {
                    self.save_undo_state();
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    self.modified = true;
                }
            }
            Command::Yank => {
                self.yank_selection();
            }
            Command::Paste { before } => {
                if before {
                    self.paste_before();
                } else {
                    self.paste_after();
                }
            }

            Command::InsertBefore => {}
            Command::InsertAfter => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, false);
                });
            }
            Command::InsertLineStart => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_first_nonwhitespace(slice, *r, false);
                });
            }
            Command::InsertLineEnd => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_end(slice, *r, false);
                });
            }
            Command::OpenBelow => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_end(slice, *r, false);
                });
                self.insert_text("\n");
            }
            Command::OpenAbove => {
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
            }

            Command::Escape => {
                self.selection.transform_mut(|r| {
                    r.anchor = r.head;
                });
            }

            Command::ScrollHalfPageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(slice, *r, MoveDir::Backward, 10, false);
                });
            }
            Command::ScrollHalfPageDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(10);
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(slice, *r, MoveDir::Forward, 10, false);
                });
            }
            Command::ScrollPageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(20);
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(slice, *r, MoveDir::Backward, 20, false);
                });
            }
            Command::ScrollPageDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(20);
                self.selection.transform_mut(|r| {
                    *r = movement::move_vertically(slice, *r, MoveDir::Forward, 20, false);
                });
            }

            Command::ToLowerCase => {
                let primary = self.selection.primary();
                let from = primary.from();
                let to = primary.to();
                if from < to {
                    self.save_undo_state();
                    let text: String = self.doc.slice(from..to).chars().flat_map(|c| c.to_lowercase()).collect();
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    // Insert without saving undo again (we saved above)
                    let tx = Transaction::insert(self.doc.slice(..), &self.selection, text);
                    tx.apply(&mut self.doc);
                    let head = self.selection.primary().head + (to - from);
                    self.selection = Selection::point(head);
                    self.modified = true;
                }
            }
            Command::ToUpperCase => {
                let primary = self.selection.primary();
                let from = primary.from();
                let to = primary.to();
                if from < to {
                    self.save_undo_state();
                    let text: String = self.doc.slice(from..to).chars().flat_map(|c| c.to_uppercase()).collect();
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    // Insert without saving undo again
                    let tx = Transaction::insert(self.doc.slice(..), &self.selection, text.clone());
                    tx.apply(&mut self.doc);
                    let head = self.selection.primary().head + text.chars().count();
                    self.selection = Selection::point(head);
                    self.modified = true;
                }
            }

            Command::JoinLines => {
                let primary = self.selection.primary();
                let line = self.doc.char_to_line(primary.head);
                if line + 1 < self.doc.len_lines() {
                    self.save_undo_state();
                    let end_of_line = self.doc.line_to_char(line + 1) - 1;
                    self.selection = Selection::single(end_of_line, end_of_line + 1);
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    // Insert without saving undo again
                    let tx = Transaction::insert(self.doc.slice(..), &self.selection, " ".to_string());
                    tx.apply(&mut self.doc);
                    let head = self.selection.primary().head + 1;
                    self.selection = Selection::point(head);
                    self.modified = true;
                }
            }

            Command::Indent => {
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_start(slice, *r, false);
                });
                self.insert_text("    ");
            }
            Command::Deindent => {
                let line = self.doc.char_to_line(self.selection.primary().head);
                let line_start = self.doc.line_to_char(line);
                let line_text: String = self.doc.line(line).chars().take(4).collect();
                let spaces = line_text.chars().take_while(|c| *c == ' ').count().min(4);
                if spaces > 0 {
                    self.save_undo_state();
                    self.selection = Selection::single(line_start, line_start + spaces);
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    self.modified = true;
                }
            }

            Command::Undo => {
                self.undo();
            }
            Command::Redo => {
                self.redo();
            }

            Command::SelectObject { object_type, selection } => {
                if let Some(obj) = object_type {
                    match selection {
                        ObjectSelection::Inner => self.select_object(obj, true),
                        ObjectSelection::Around => self.select_object(obj, false),
                        ObjectSelection::ToStart => self.select_to_object_boundary(obj, true, extend),
                        ObjectSelection::ToEnd => self.select_to_object_boundary(obj, false, extend),
                    }
                }
            }

            _ => {
                self.message = Some(format!("{:?} not implemented", cmd));
            }
        }

        false
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        self.message = None;

        let key: Key = key.into();
        let result = self.input.handle_key(key);

        match result {
            KeyResult::Command(cmd, params) => {
                self.execute_command(cmd, params.count, params.extend)
            }
            KeyResult::ModeChange(mode) => {
                if matches!(mode, Mode::Normal) {
                    self.message = None;
                }
                false
            }
            KeyResult::InsertChar(c) => {
                self.insert_text(&c.to_string());
                false
            }
            KeyResult::Pending(msg) => {
                self.message = Some(msg);
                false
            }
            KeyResult::ExecuteCommand(cmd) => self.execute_command_line(&cmd),
            KeyResult::ExecuteSearch { pattern, reverse } => {
                self.message = Some(format!(
                    "Search '{}' {}",
                    pattern,
                    if reverse { "(reverse)" } else { "" }
                ));
                false
            }
            KeyResult::Consumed => false,
            KeyResult::Unhandled => false,
            KeyResult::Quit => true,
        }
    }

    fn render_document(&self, area: Rect) -> impl Widget + '_ {
        // Calculate gutter width based on max line number
        let total_lines = self.doc.len_lines();
        let gutter_width = total_lines.max(1).ilog10() as u16 + 2; // +2 for padding
        let text_width = area.width.saturating_sub(gutter_width) as usize;

        let primary = self.selection.primary();
        let sel_start = primary.from();
        let sel_end = primary.to();

        let mut output_lines: Vec<Line> = Vec::new();
        let mut current_line_idx = self.scroll_offset;
        let viewport_height = area.height as usize;

        // Process document lines until we fill the viewport
        while output_lines.len() < viewport_height && current_line_idx < total_lines {
            let line_start = self.doc.line_to_char(current_line_idx);
            let line_end = if current_line_idx + 1 < total_lines {
                self.doc.line_to_char(current_line_idx + 1)
            } else {
                self.doc.len_chars()
            };

            let line_text: String = self.doc.slice(line_start..line_end).into();
            let line_text = line_text.trim_end_matches('\n');

            // Split line into wrapped segments with word-boundary intelligence
            let wrapped_segments = self.wrap_line(line_text, text_width);
            let num_segments = wrapped_segments.len().max(1);

            for (seg_idx, segment) in wrapped_segments.iter().enumerate() {
                if output_lines.len() >= viewport_height {
                    break;
                }

                let is_first_segment = seg_idx == 0;
                let is_last_segment = seg_idx == num_segments - 1;

                // Build gutter span
                let line_num_str = if is_first_segment {
                    format!("{:>width$} ", current_line_idx + 1, width = gutter_width as usize - 1)
                } else {
                    format!("{:>width$} ", "┆", width = gutter_width as usize - 1)
                };
                let gutter_style = if is_first_segment {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Rgb(60, 60, 60)) // Dimmer for wrapped lines
                };

                let mut spans = vec![Span::styled(line_num_str, gutter_style)];

                // Build content spans for this segment
                let seg_char_offset = segment.start_offset;
                for (i, ch) in segment.text.chars().enumerate() {
                    let doc_pos = line_start + seg_char_offset + i;
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

                // Handle cursor at end of line (only on last segment)
                if is_last_segment {
                    let line_content_end = line_start + line_text.chars().count();
                    let is_last_doc_line = current_line_idx + 1 >= total_lines;
                    let cursor_at_eol = if is_last_doc_line {
                        primary.head >= line_content_end && primary.head <= line_end
                    } else {
                        primary.head >= line_content_end && primary.head < line_end
                    };
                    if cursor_at_eol && primary.head >= line_content_end {
                        spans.push(Span::styled(
                            " ",
                            Style::default()
                                .bg(Color::White)
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                }

                output_lines.push(Line::from(spans));
            }

            // Handle empty lines (no segments)
            if wrapped_segments.is_empty() {
                if output_lines.len() < viewport_height {
                    let line_num_str = format!("{:>width$} ", current_line_idx + 1, width = gutter_width as usize - 1);
                    let gutter_style = Style::default().fg(Color::DarkGray);
                    let mut spans = vec![Span::styled(line_num_str, gutter_style)];

                    // Cursor on empty line
                    let is_last_doc_line = current_line_idx + 1 >= total_lines;
                    let cursor_at_eol = if is_last_doc_line {
                        primary.head >= line_start && primary.head <= line_end
                    } else {
                        primary.head >= line_start && primary.head < line_end
                    };
                    if cursor_at_eol {
                        spans.push(Span::styled(
                            " ",
                            Style::default()
                                .bg(Color::White)
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }

                    output_lines.push(Line::from(spans));
                }
            }

            current_line_idx += 1;
        }

        // Fill remaining viewport with empty lines (tildes like vim, or just empty)
        while output_lines.len() < viewport_height {
            let line_num_str = format!("{:>width$} ", "~", width = gutter_width as usize - 1);
            let gutter_style = Style::default().fg(Color::Rgb(60, 60, 60));
            output_lines.push(Line::from(vec![Span::styled(line_num_str, gutter_style)]));
        }

        Paragraph::new(output_lines)
    }

    /// Wrap a line of text into segments that fit within the given width.
    /// Uses word-boundary intelligence to break at word boundaries when possible.
    fn wrap_line(&self, line: &str, max_width: usize) -> Vec<WrapSegment> {
        if max_width == 0 {
            return vec![];
        }

        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            return vec![];
        }

        let mut segments = Vec::new();
        let mut pos = 0;

        while pos < chars.len() {
            let remaining = chars.len() - pos;
            if remaining <= max_width {
                // Rest of line fits
                segments.push(WrapSegment {
                    text: chars[pos..].iter().collect(),
                    start_offset: pos,
                });
                break;
            }

            // Need to wrap - find best break point
            let segment_end = pos + max_width;
            let break_pos = self.find_wrap_break(&chars, pos, segment_end);

            segments.push(WrapSegment {
                text: chars[pos..break_pos].iter().collect(),
                start_offset: pos,
            });

            pos = break_pos;
            // Skip leading whitespace on wrapped lines
            while pos < chars.len() && chars[pos] == ' ' {
                pos += 1;
            }
        }

        segments
    }

    /// Find the best position to break a line for wrapping.
    /// Prefers breaking at word boundaries (after spaces, before certain punctuation).
    fn find_wrap_break(&self, chars: &[char], start: usize, max_end: usize) -> usize {
        // Look backwards from max_end to find a good break point
        let search_start = start + (max_end - start) / 2; // Don't search too far back

        for i in (search_start..max_end).rev() {
            let ch = chars[i];
            // Break after whitespace
            if ch == ' ' || ch == '\t' {
                return i + 1;
            }
            // Break before certain punctuation
            if i + 1 < chars.len() {
                let next = chars[i + 1];
                if next == '-' || next == '/' || next == '.' || next == ',' {
                    return i + 1;
                }
            }
        }

        // No good break point found, hard break at max width
        max_end
    }

    fn render_status_line(&self) -> impl Widget + '_ {
        let mode_style = match self.mode() {
            Mode::Normal => Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            Mode::Insert => Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            Mode::Goto => Style::default()
                .bg(Color::Magenta)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            Mode::View => Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            Mode::Command { .. } => Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            Mode::Pending(_) => Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        };

        let modified = if self.modified { " [+]" } else { "" };
        let path = self.path.display().to_string();
        let cursor_info = format!(" {}:{} ", self.cursor_line() + 1, self.cursor_col() + 1);

        let count_str = if self.input.count() > 0 {
            format!(" {} ", self.input.count())
        } else {
            String::new()
        };

        let spans = vec![
            Span::styled(format!(" {} ", self.mode_name()), mode_style),
            Span::raw(count_str),
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
        // Show command line input if in command mode
        if let Some((prompt, input)) = self.input.command_line() {
            return Paragraph::new(format!("{}{}", prompt, input))
                .style(Style::default().fg(Color::White));
        }
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
                // Only handle key press events, not release or repeat
                if key.kind == crossterm::event::KeyEventKind::Press {
                    if editor.handle_key(key) {
                        break;
                    }
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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
        // Press Shift+L three times to extend selection
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
        assert_eq!(editor.selection.primary().head, 6); // at 'w'
    }

    #[test]
    fn test_goto_mode() {
        let mut editor = test_editor("line1\nline2\nline3");
        // g then g should go to start
        editor.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(matches!(editor.mode(), Mode::Goto));
        editor.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().head, 0);
    }

    #[test]
    fn test_undo_redo() {
        let mut editor = test_editor("hello");
        assert_eq!(editor.doc.to_string(), "hello");

        // Select all and delete
        editor.handle_key(KeyEvent::new(KeyCode::Char('%'), KeyModifiers::NONE)); // select all
        assert_eq!(editor.selection.primary().from(), 0);
        assert_eq!(editor.selection.primary().to(), 5);

        editor.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)); // delete
        assert_eq!(editor.doc.to_string(), "", "after delete");
        assert_eq!(editor.undo_stack.len(), 1, "undo stack should have 1 entry");

        // Undo should restore
        editor.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
        assert_eq!(editor.doc.to_string(), "hello", "after undo");
        assert_eq!(editor.redo_stack.len(), 1, "redo stack should have 1 entry");
        assert_eq!(editor.undo_stack.len(), 0, "undo stack should be empty");

        // Redo should delete again
        editor.handle_key(KeyEvent::new(KeyCode::Char('U'), KeyModifiers::SHIFT));
        assert_eq!(editor.redo_stack.len(), 0, "redo stack should be empty after redo");
        assert_eq!(editor.doc.to_string(), "", "after redo");
    }

    #[test]
    fn test_insert_newline_single_cursor() {
        // Bug: in empty file, enter insert mode, press Enter creates 2 blank lines
        // with cursors appearing on BOTH lines instead of just the second line
        use ratatui::style::{Color, Modifier};
        
        let mut editor = test_editor("");
        
        // Enter insert mode
        editor.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        assert!(matches!(editor.mode(), Mode::Insert));
        
        // Press Enter to create a new line
        editor.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        
        // Should have 2 lines now (empty first line + cursor on second)
        assert_eq!(editor.doc.len_lines(), 2, "should have 2 lines after Enter");
        
        // Cursor should be at position 1 (after the newline)
        assert_eq!(editor.selection.primary().head, 1, "cursor should be at position 1");
        
        // Render
        let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        
        // Count cells with cursor styling (white bg, black fg, bold)
        // The cursor style is: bg=White, fg=Black, bold
        let buffer = terminal.backend().buffer();
        let mut cursor_cells = Vec::new();
        for row in 0..8 {  // Only check document area, not status line
            for col in 0..80 {
                let cell = &buffer[(col, row)];
                if cell.bg == Color::White && cell.fg == Color::Black 
                   && cell.modifier.contains(Modifier::BOLD) {
                    cursor_cells.push((col, row));
                }
            }
        }
        
        // Should have exactly ONE cursor cell, on line 2 (row index 1)
        // With gutter, cursor is offset by gutter width (2 chars for "1 " or "2 ")
        assert_eq!(
            cursor_cells.len(), 
            1, 
            "Expected 1 cursor cell, found {} at positions: {:?}", 
            cursor_cells.len(), 
            cursor_cells
        );
        // Cursor at column 2 (after gutter), row 1 (second line)
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

        // Enter insert mode with 'i'
        editor.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        assert!(matches!(editor.mode(), Mode::Insert), "should be in insert mode");

        // Move right with arrow key
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().head, 1, "after Right arrow, cursor at 1");

        // Move right again
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().head, 2, "after Right arrow, cursor at 2");

        // Move left
        editor.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(editor.selection.primary().head, 1, "after Left arrow, cursor at 1");

        // Move down (should go to next line if we had one, but we don't - stays same)
        editor.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        // Still in insert mode
        assert!(matches!(editor.mode(), Mode::Insert), "still in insert mode after arrows");
    }

    #[test]
    fn test_soft_wrap_long_line() {
        // Create a line that's longer than the viewport width
        // With 40 char width terminal and ~2 char gutter, text area is ~38 chars
        let long_line = "The quick brown fox jumps over the lazy dog and keeps on running";
        let mut editor = test_editor(long_line);
        
        // Use a narrow terminal to force wrapping
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_soft_wrap_word_boundary() {
        // Test that wrapping breaks at word boundaries, not mid-word
        let text = "hello world this is a test of word wrapping behavior";
        let mut editor = test_editor(text);
        
        let mut terminal = Terminal::new(TestBackend::new(30, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_line_numbers_multiple_lines() {
        let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let editor = test_editor(text);
        
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_wrapped_line_dim_gutter() {
        use ratatui::style::Color;
        
        // Long line that will wrap
        let long_line = "This is a very long line that should wrap to multiple virtual lines";
        let editor = test_editor(long_line);
        
        let mut terminal = Terminal::new(TestBackend::new(30, 10)).unwrap();
        terminal.draw(|frame| editor.render(frame)).unwrap();
        
        let buffer = terminal.backend().buffer();
        
        // First line gutter should be DarkGray (the normal line number color)
        let first_gutter = &buffer[(0, 0)];
        assert_eq!(first_gutter.fg, Color::DarkGray, "first line gutter should be DarkGray");
        
        // Second line (wrapped continuation) gutter should be dimmer (Rgb(60,60,60))
        let second_gutter = &buffer[(0, 1)];
        assert_eq!(second_gutter.fg, Color::Rgb(60, 60, 60), "wrapped line gutter should be dim");
        
        assert_snapshot!(terminal.backend());
    }
}
