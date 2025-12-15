use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use tome_core::range::Direction as MoveDir;
use tome_core::{
    InputHandler, Key, KeyResult, Mode, MouseEvent, Rope, ScrollDirection, Selection, Transaction,
    ext, movement,
};
use tome_core::ext::{HookContext, emit_hook};

use crate::commands;
use crate::render::WrapSegment;

/// A history entry for undo/redo.
#[derive(Clone)]
pub struct HistoryEntry {
    pub doc: Rope,
    pub selection: Selection,
}

#[derive(Default)]
pub struct Registers {
    pub yank: String,
}

pub struct Editor {
    pub doc: Rope,
    pub selection: Selection,
    pub input: InputHandler,
    pub path: Option<PathBuf>,
    pub modified: bool,
    pub scroll_line: usize,
    pub scroll_segment: usize,
    pub message: Option<String>,
    pub registers: Registers,
    pub undo_stack: Vec<HistoryEntry>,
    pub redo_stack: Vec<HistoryEntry>,
    pub text_width: usize,
}

impl Editor {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        let content = if path.exists() {
            fs::read_to_string(&path)?
        } else {
            String::new()
        };

        Ok(Self::from_content(content, Some(path)))
    }

    pub fn new_scratch() -> Self {
        Self::from_content(String::new(), None)
    }

    pub fn from_content(content: String, path: Option<PathBuf>) -> Self {
        let file_type = path
            .as_ref()
            .and_then(|p| ext::detect_file_type(p.to_str().unwrap_or("")))
            .map(|ft| ft.name);

        let doc = Rope::from(content.as_str());

        let scratch_path = PathBuf::from("[scratch]");
        let hook_path = path.as_ref().unwrap_or(&scratch_path);

        emit_hook(&HookContext::BufferOpen {
            path: hook_path,
            text: doc.slice(..),
            file_type,
        });

        Self {
            doc,
            selection: Selection::point(0),
            input: InputHandler::new(),
            path,
            modified: false,
            scroll_line: 0,
            scroll_segment: 0,
            message: None,
            registers: Registers::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            text_width: 80,
        }
    }

    pub fn mode(&self) -> Mode {
        self.input.mode()
    }

    pub fn mode_name(&self) -> &'static str {
        self.input.mode_name()
    }

    pub fn cursor_line(&self) -> usize {
        let head = self.selection.primary().head;
        self.doc
            .char_to_line(head.min(self.doc.len_chars().saturating_sub(1).max(0)))
    }

    pub fn cursor_col(&self) -> usize {
        let head = self.selection.primary().head;
        let line = self.cursor_line();
        let line_start = self.doc.line_to_char(line);
        head.saturating_sub(line_start)
    }

    pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
        for _ in 0..count {
            let head = self.selection.primary().head;
            let doc_line = self.doc.char_to_line(head);
            let line_start = self.doc.line_to_char(doc_line);
            let col_in_line = head - line_start;

            let total_lines = self.doc.len_lines();
            let line_end = if doc_line + 1 < total_lines {
                self.doc.line_to_char(doc_line + 1)
            } else {
                self.doc.len_chars()
            };
            let line_text: String = self.doc.slice(line_start..line_end).into();
            let line_text = line_text.trim_end_matches('\n');

            let segments = self.wrap_line(line_text, self.text_width);
            let current_seg_idx = self.find_segment_for_col(&segments, col_in_line);
            let col_in_seg = if current_seg_idx < segments.len() {
                col_in_line.saturating_sub(segments[current_seg_idx].start_offset)
            } else {
                col_in_line
            };

            let new_pos = match direction {
                MoveDir::Forward => {
                    if current_seg_idx + 1 < segments.len() {
                        let next_seg = &segments[current_seg_idx + 1];
                        let new_col = next_seg.start_offset + col_in_seg.min(next_seg.text.chars().count().saturating_sub(1));
                        line_start + new_col
                    } else if doc_line + 1 < total_lines {
                        let next_line_start = self.doc.line_to_char(doc_line + 1);
                        let next_line_end = if doc_line + 2 < total_lines {
                            self.doc.line_to_char(doc_line + 2)
                        } else {
                            self.doc.len_chars()
                        };
                        let next_line_text: String = self.doc.slice(next_line_start..next_line_end).into();
                        let next_line_text = next_line_text.trim_end_matches('\n');
                        let next_segments = self.wrap_line(next_line_text, self.text_width);

                        if next_segments.is_empty() {
                            next_line_start
                        } else {
                            let first_seg = &next_segments[0];
                            let new_col = col_in_seg.min(first_seg.text.chars().count().saturating_sub(1).max(0));
                            next_line_start + new_col
                        }
                    } else {
                        head
                    }
                }
                MoveDir::Backward => {
                    if current_seg_idx > 0 {
                        let prev_seg = &segments[current_seg_idx - 1];
                        let new_col = prev_seg.start_offset + col_in_seg.min(prev_seg.text.chars().count().saturating_sub(1));
                        line_start + new_col
                    } else if doc_line > 0 {
                        let prev_line = doc_line - 1;
                        let prev_line_start = self.doc.line_to_char(prev_line);
                        let prev_line_end = line_start;
                        let prev_line_text: String = self.doc.slice(prev_line_start..prev_line_end).into();
                        let prev_line_text = prev_line_text.trim_end_matches('\n');
                        let prev_segments = self.wrap_line(prev_line_text, self.text_width);

                        if prev_segments.is_empty() {
                            prev_line_start
                        } else {
                            let last_seg = &prev_segments[prev_segments.len() - 1];
                            let new_col = last_seg.start_offset + col_in_seg.min(last_seg.text.chars().count().saturating_sub(1).max(0));
                            prev_line_start + new_col
                        }
                    } else {
                        head
                    }
                }
            };

            self.selection.transform_mut(|r| {
                if extend {
                    r.head = new_pos;
                } else {
                    r.anchor = new_pos;
                    r.head = new_pos;
                }
            });
        }
    }

    pub fn find_segment_for_col(&self, segments: &[WrapSegment], col: usize) -> usize {
        for (i, seg) in segments.iter().enumerate() {
            let seg_end = seg.start_offset + seg.text.chars().count();
            if col < seg_end || i == segments.len() - 1 {
                return i;
            }
        }
        0
    }

    pub fn save_undo_state(&mut self) {
        self.undo_stack.push(HistoryEntry {
            doc: self.doc.clone(),
            selection: self.selection.clone(),
        });
        self.redo_stack.clear();

        const MAX_UNDO: usize = 100;
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
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

    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
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

    pub fn insert_text(&mut self, text: &str) {
        self.save_undo_state();
        let tx = Transaction::insert(self.doc.slice(..), &self.selection, text.to_string());
        tx.apply(&mut self.doc);
        let head = self.selection.primary().head + text.chars().count();
        self.selection = Selection::point(head);
        self.modified = true;
    }

    pub fn save(&mut self) -> io::Result<()> {
        let path = match &self.path {
            Some(p) => p,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "No filename. Use :write <filename>",
                ));
            }
        };

        emit_hook(&HookContext::BufferWritePre {
            path,
            text: self.doc.slice(..),
        });

        let mut f = fs::File::create(path)?;
        for chunk in self.doc.chunks() {
            f.write_all(chunk.as_bytes())?;
        }
        self.modified = false;
        self.message = Some(format!("Saved {}", path.display()));

        emit_hook(&HookContext::BufferWrite { path });

        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
        self.path = Some(path);
        self.save()
    }

    pub fn yank_selection(&mut self) {
        let primary = self.selection.primary();
        let from = primary.from();
        let to = primary.to();
        if from < to {
            self.registers.yank = self.doc.slice(from..to).to_string();
            self.message = Some(format!("Yanked {} chars", to - from));
        }
    }

    pub fn paste_after(&mut self) {
        if self.registers.yank.is_empty() {
            return;
        }
        let slice = self.doc.slice(..);
        self.selection.transform_mut(|r| {
            *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, false);
        });
        self.insert_text(&self.registers.yank.clone());
    }

    pub fn paste_before(&mut self) {
        if self.registers.yank.is_empty() {
            return;
        }
        self.insert_text(&self.registers.yank.clone());
    }

    /// Select a text object using the extension registry.
    pub fn select_object_by_trigger(&mut self, trigger: char, inner: bool) -> bool {
        if let Some(obj_def) = ext::find_text_object(trigger) {
            let slice = self.doc.slice(..);
            self.selection.transform_mut(|r| {
                let handler = if inner { obj_def.inner } else { obj_def.around };
                if let Some(new_range) = handler(slice, r.head) {
                    *r = new_range;
                }
            });
            true
        } else {
            false
        }
    }

    /// Select to object boundary using the extension registry.
    pub fn select_to_object_boundary(&mut self, trigger: char, to_start: bool, extend: bool) -> bool {
        if let Some(obj_def) = ext::find_text_object(trigger) {
            let slice = self.doc.slice(..);
            self.selection.transform_mut(|r| {
                // Use 'around' to get the full object bounds
                if let Some(obj_range) = (obj_def.around)(slice, r.head) {
                    let new_head = if to_start { obj_range.from() } else { obj_range.to() };
                    if extend {
                        *r = tome_core::Range::new(r.anchor, new_head);
                    } else {
                        *r = tome_core::Range::new(r.head, new_head);
                    }
                }
            });
            true
        } else {
            false
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        self.message = None;

        let old_mode = self.mode();
        let key: Key = key.into();
        let result = self.input.handle_key(key);

        match result {
            KeyResult::Command(cmd, params) => {
                commands::execute_command(self, cmd, params.count, params.extend)
            }
            KeyResult::ModeChange(new_mode) => {
                let is_normal = matches!(new_mode, Mode::Normal);
                if new_mode != old_mode {
                    emit_hook(&HookContext::ModeChange {
                        old_mode,
                        new_mode,
                    });
                }
                if is_normal {
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
            KeyResult::ExecuteCommand(cmd) => commands::execute_command_line(self, &cmd),
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
            KeyResult::MouseClick { row, col, extend } => {
                self.handle_mouse_click(row, col, extend);
                false
            }
            KeyResult::MouseDrag { row, col } => {
                self.handle_mouse_drag(row, col);
                false
            }
            KeyResult::MouseScroll { direction, count } => {
                self.handle_mouse_scroll(direction, count);
                false
            }
        }
    }

    pub fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> bool {
        self.message = None;
        let event: MouseEvent = mouse.into();
        let result = self.input.handle_mouse(event);

        match result {
            KeyResult::MouseClick { row, col, extend } => {
                self.handle_mouse_click(row, col, extend);
                false
            }
            KeyResult::MouseDrag { row, col } => {
                self.handle_mouse_drag(row, col);
                false
            }
            KeyResult::MouseScroll { direction, count } => {
                self.handle_mouse_scroll(direction, count);
                false
            }
            KeyResult::Consumed => false,
            _ => false,
        }
    }

    fn handle_mouse_click(&mut self, screen_row: u16, screen_col: u16, extend: bool) {
        if let Some(doc_pos) = self.screen_to_doc_position(screen_row, screen_col) {
            if extend {
                let anchor = self.selection.primary().anchor;
                self.selection = Selection::single(anchor, doc_pos);
            } else {
                self.selection = Selection::point(doc_pos);
            }
        }
    }

    fn handle_mouse_drag(&mut self, screen_row: u16, screen_col: u16) {
        if let Some(doc_pos) = self.screen_to_doc_position(screen_row, screen_col) {
            let anchor = self.selection.primary().anchor;
            self.selection = Selection::single(anchor, doc_pos);
        }
    }

    fn handle_mouse_scroll(&mut self, direction: ScrollDirection, count: usize) {
        match direction {
            ScrollDirection::Up => {
                for _ in 0..count {
                    if self.scroll_segment > 0 {
                        self.scroll_segment -= 1;
                    } else if self.scroll_line > 0 {
                        self.scroll_line -= 1;
                        let line_start = self.doc.line_to_char(self.scroll_line);
                        let line_end = if self.scroll_line + 1 < self.doc.len_lines() {
                            self.doc.line_to_char(self.scroll_line + 1)
                        } else {
                            self.doc.len_chars()
                        };
                        let line_text: String = self.doc.slice(line_start..line_end).into();
                        let line_text = line_text.trim_end_matches('\n');
                        let segments = self.wrap_line(line_text, self.text_width);
                        self.scroll_segment = segments.len().saturating_sub(1);
                    }
                }
            }
            ScrollDirection::Down => {
                for _ in 0..count {
                    let total_lines = self.doc.len_lines();
                    if self.scroll_line < total_lines {
                        let line_start = self.doc.line_to_char(self.scroll_line);
                        let line_end = if self.scroll_line + 1 < total_lines {
                            self.doc.line_to_char(self.scroll_line + 1)
                        } else {
                            self.doc.len_chars()
                        };
                        let line_text: String = self.doc.slice(line_start..line_end).into();
                        let line_text = line_text.trim_end_matches('\n');
                        let segments = self.wrap_line(line_text, self.text_width);
                        let num_segments = segments.len().max(1);

                        if self.scroll_segment + 1 < num_segments {
                            self.scroll_segment += 1;
                        } else if self.scroll_line + 1 < total_lines {
                            self.scroll_line += 1;
                            self.scroll_segment = 0;
                        }
                    }
                }
            }
            ScrollDirection::Left | ScrollDirection::Right => {
                // Horizontal scroll not implemented yet
            }
        }
    }

    fn screen_to_doc_position(&self, screen_row: u16, screen_col: u16) -> Option<usize> {
        let total_lines = self.doc.len_lines();
        let gutter_width = total_lines.max(1).ilog10() as u16 + 2;

        if screen_col < gutter_width {
            return None;
        }

        let text_col = (screen_col - gutter_width) as usize;
        let mut visual_row = 0;
        let mut line_idx = self.scroll_line;
        let mut start_segment = self.scroll_segment;

        while line_idx < total_lines {
            let line_start = self.doc.line_to_char(line_idx);
            let line_end = if line_idx + 1 < total_lines {
                self.doc.line_to_char(line_idx + 1)
            } else {
                self.doc.len_chars()
            };

            let line_text: String = self.doc.slice(line_start..line_end).into();
            let line_text = line_text.trim_end_matches('\n');
            let segments = self.wrap_line(line_text, self.text_width);

            if segments.is_empty() {
                if visual_row == screen_row as usize {
                    return Some(line_start);
                }
                visual_row += 1;
            } else {
                for (_seg_idx, segment) in segments.iter().enumerate().skip(start_segment) {
                    if visual_row == screen_row as usize {
                        let seg_len = segment.text.chars().count();
                        let col_in_seg = text_col.min(seg_len.saturating_sub(1).max(0));
                        return Some(line_start + segment.start_offset + col_in_seg);
                    }
                    visual_row += 1;
                }
            }

            start_segment = 0;
            line_idx += 1;
        }

        Some(self.doc.len_chars().saturating_sub(1).max(0))
    }
}

impl ext::EditorOps for Editor {
    fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    fn text(&self) -> tome_core::RopeSlice<'_> {
        self.doc.slice(..)
    }

    fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    fn message(&mut self, msg: &str) {
        self.message = Some(msg.to_string());
    }

    fn error(&mut self, msg: &str) {
        self.message = Some(msg.to_string());
    }

    fn save(&mut self) -> Result<(), ext::CommandError> {
        Editor::save(self).map_err(|e| ext::CommandError::Io(e.to_string()))
    }

    fn save_as(&mut self, path: std::path::PathBuf) -> Result<(), ext::CommandError> {
        Editor::save_as(self, path).map_err(|e| ext::CommandError::Io(e.to_string()))
    }

    fn insert_text(&mut self, text: &str) {
        Editor::insert_text(self, text);
    }

    fn delete_selection(&mut self) {
        if !self.selection.primary().is_empty() {
            self.save_undo_state();
            let tx = Transaction::delete(self.doc.slice(..), &self.selection);
            self.selection = tx.map_selection(&self.selection);
            tx.apply(&mut self.doc);
            self.modified = true;
        }
    }

    fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    fn is_modified(&self) -> bool {
        self.modified
    }
}
