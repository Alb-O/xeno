use std::fs;
use std::io::{self, Write};
use std::mem;
use std::path::PathBuf;

use tome_core::key::{KeyCode, SpecialKey};
use tome_core::range::Direction as MoveDir;
use tome_core::{
    InputHandler, Key, KeyResult, Mode, MouseEvent, Rope, ScrollDirection, Selection, Transaction,
    ext, movement,
};
use tome_core::ext::{HookContext, emit_hook};

use crate::render::WrapSegment;
use crate::theme::{self, Theme};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MessageKind {
    Info,
    Error,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub text: String,
    pub kind: MessageKind,
}

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

#[derive(Clone)]
pub struct ScratchState {
    pub doc: Rope,
    pub cursor: usize,
    pub selection: Selection,
    pub input: InputHandler,
    pub path: Option<PathBuf>,
    pub modified: bool,
    pub scroll_line: usize,
    pub scroll_segment: usize,
    pub undo_stack: Vec<HistoryEntry>,
    pub redo_stack: Vec<HistoryEntry>,
    pub text_width: usize,
}

impl Default for ScratchState {

    fn default() -> Self {
        Self {
            doc: Rope::from(""),
            cursor: 0,
            selection: Selection::point(0),
            input: InputHandler::new(),
            path: None,
            modified: false,
            scroll_line: 0,
            scroll_segment: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            text_width: 80,
        }
    }
}

pub struct Editor {
    pub doc: Rope,
    pub cursor: usize,
    pub selection: Selection,
    pub input: InputHandler,
    pub path: Option<PathBuf>,
    pub modified: bool,
    pub scroll_line: usize,
    pub scroll_segment: usize,
    pub message: Option<Message>,
    pub registers: Registers,
    pub undo_stack: Vec<HistoryEntry>,
    pub redo_stack: Vec<HistoryEntry>,
    pub text_width: usize,
    pub scratch: ScratchState,
    pub scratch_open: bool,
    pub scratch_height: u16,
    pub scratch_keep_open: bool,
    pub scratch_focused: bool,
    in_scratch_context: bool,
    pub file_type: Option<String>,
    pub theme: &'static Theme,
}

impl Editor {
    pub fn show_message(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            kind: MessageKind::Info,
        });
    }

    pub fn show_error(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            kind: MessageKind::Error,
        });
    }

    fn execute_command_line(&mut self, input: &str) -> bool {

        use ext::{find_command, CommandContext, CommandOutcome};

        let trimmed = input.trim();
        if trimmed.is_empty() {
            return false;
        }

        let mut parts = trimmed.split_whitespace();
        let name = match parts.next() {
            Some(n) => n,
            None => return false,
        };

        let arg_strings: Vec<String> = parts.map(|s| s.to_string()).collect();
        let args: Vec<&str> = arg_strings.iter().map(|s| s.as_str()).collect();

        let cmd = match find_command(name) {
            Some(cmd) => cmd,
            None => {
                self.show_error(format!("Unknown command: {}", name));
                return false;
            }
        };

        let mut ctx = CommandContext {
            editor: self,
            args: &args,
            count: 1,
            register: None,
        };

        match (cmd.handler)(&mut ctx) {
            Ok(CommandOutcome::Ok) => false,
            Ok(CommandOutcome::Quit) => true,
            Ok(CommandOutcome::ForceQuit) => true,
            Err(e) => {
                ctx.editor.error(&e.to_string());
                false
            }
        }
    }
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
            cursor: 0,
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
            scratch: ScratchState::default(),
            scratch_open: false,
            scratch_height: 6,
            scratch_keep_open: true,
            scratch_focused: false,
            in_scratch_context: false,
            file_type: file_type.map(|s| s.to_string()),
            theme: &crate::themes::solarized::SOLARIZED_DARK,
        }
    }

    pub fn mode(&self) -> Mode {
        self.input.mode()
    }

    #[cfg(test)]
    pub(crate) fn in_scratch_context(&self) -> bool {
        self.in_scratch_context
    }

    pub fn mode_name(&self) -> &'static str {
        self.input.mode_name()
    }

    pub(crate) fn enter_scratch_context(&mut self) {
        if self.in_scratch_context {
            return;
        }
        self.in_scratch_context = true;
        mem::swap(&mut self.doc, &mut self.scratch.doc);
        mem::swap(&mut self.cursor, &mut self.scratch.cursor);
        mem::swap(&mut self.selection, &mut self.scratch.selection);
        mem::swap(&mut self.input, &mut self.scratch.input);
        mem::swap(&mut self.path, &mut self.scratch.path);
        mem::swap(&mut self.modified, &mut self.scratch.modified);
        mem::swap(&mut self.scroll_line, &mut self.scratch.scroll_line);
        mem::swap(&mut self.scroll_segment, &mut self.scratch.scroll_segment);
        mem::swap(&mut self.undo_stack, &mut self.scratch.undo_stack);
        mem::swap(&mut self.redo_stack, &mut self.scratch.redo_stack);
        mem::swap(&mut self.text_width, &mut self.scratch.text_width);
    }

    pub(crate) fn leave_scratch_context(&mut self) {
        if !self.in_scratch_context {
            return;
        }
        self.in_scratch_context = false;
        mem::swap(&mut self.doc, &mut self.scratch.doc);
        mem::swap(&mut self.cursor, &mut self.scratch.cursor);
        mem::swap(&mut self.selection, &mut self.scratch.selection);
        mem::swap(&mut self.input, &mut self.scratch.input);
        mem::swap(&mut self.path, &mut self.scratch.path);
        mem::swap(&mut self.modified, &mut self.scratch.modified);
        mem::swap(&mut self.scroll_line, &mut self.scratch.scroll_line);
        mem::swap(&mut self.scroll_segment, &mut self.scratch.scroll_segment);
        mem::swap(&mut self.undo_stack, &mut self.scratch.undo_stack);
        mem::swap(&mut self.redo_stack, &mut self.scratch.redo_stack);
        mem::swap(&mut self.text_width, &mut self.scratch.text_width);
    }

    pub(crate) fn with_scratch_context<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.enter_scratch_context();
        let result = f(self);
        self.leave_scratch_context();
        result
    }

    pub(crate) fn do_open_scratch(&mut self, focus: bool) {
        self.scratch_open = true;
        if focus {
            self.scratch_focused = true;
            self.with_scratch_context(|ed| {
                if ed.doc.len_chars() == 0 {
                    ed.cursor = 0;
                    ed.selection = Selection::point(0);
                }
                ed.input.set_mode(Mode::Insert);
            });
        }
    }

    pub(crate) fn do_close_scratch(&mut self) {
        if self.in_scratch_context {
            self.leave_scratch_context();
        }
        self.scratch_open = false;
        self.scratch_focused = false;
    }

    pub(crate) fn do_toggle_scratch(&mut self) {
        if !self.scratch_open {
            self.do_open_scratch(true);
        } else if self.scratch_focused {
            self.do_close_scratch();
        } else {
            self.scratch_focused = true;
        }
    }

    pub(crate) fn do_execute_scratch(&mut self) -> bool {
        if !self.scratch_open {
            self.show_error("Scratch is not open");
            return false;
        }

        let text = self.with_scratch_context(|ed| ed.doc.slice(..).to_string());
        let flattened = text
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let trimmed = flattened.trim();
        if trimmed.is_empty() {
            self.show_error("Scratch buffer is empty");
            return false;
        }

        let command = if let Some(stripped) = trimmed.strip_prefix(':') {
            stripped.trim_start()
        } else {
            trimmed
        };
        
        // Alias 'exit' to 'quit' if needed, or just rely on execute_command_line
        if command == "exit" {
             return true; 
        }

        let result = self.execute_command_line(command);
        
        if !self.scratch_keep_open {
            self.do_close_scratch();
        }
        
        result
    }

    pub fn cursor_line(&self) -> usize {
        let max_pos = self.doc.len_chars();
        self.doc.char_to_line(self.cursor.min(max_pos))
    }

    pub fn cursor_col(&self) -> usize {
        let line = self.cursor_line();
        let line_start = self.doc.line_to_char(line);
        self.cursor.saturating_sub(line_start)
    }

    /// Minimum gutter width padding (extra digits reserved beyond current line count).
    const GUTTER_MIN_WIDTH: u16 = 4;

    /// Compute the gutter width based on total line count.
    pub fn gutter_width(&self) -> u16 {
        let total_lines = self.doc.len_lines();
        (total_lines.max(1).ilog10() as u16 + 2).max(Self::GUTTER_MIN_WIDTH)
    }

    pub fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
        for _ in 0..count {
            let cursor = self.cursor;
            let doc_line = self.doc.char_to_line(cursor);
            let line_start = self.doc.line_to_char(doc_line);
            let col_in_line = cursor - line_start;

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
                        cursor
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
                        cursor
                    }
                }
            };

            self.cursor = new_pos;
            if extend {
                self.selection.transform_mut(|r| {
                    r.head = new_pos;
                });
            }
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
            self.message = Some(Message { text: "Undo".to_string(), kind: MessageKind::Info });
        } else {
            self.message = Some(Message { text: "Nothing to undo".to_string(), kind: MessageKind::Info });
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
            self.message = Some(Message { text: "Redo".to_string(), kind: MessageKind::Info });
        } else {
            self.message = Some(Message { text: "Nothing to redo".to_string(), kind: MessageKind::Info });
        }
    }

    pub fn insert_text(&mut self, text: &str) {
        self.save_undo_state();
        // Insert at cursor position
        let cursor_sel = Selection::point(self.cursor);
        let tx = Transaction::insert(self.doc.slice(..), &cursor_sel, text.to_string());
        tx.apply(&mut self.doc);
        self.cursor += text.chars().count();
        self.selection = tx.map_selection(&self.selection);
        self.modified = true;
    }

    pub fn save(&mut self) -> io::Result<()> {
        let path_owned = match &self.path {
            Some(p) => p.clone(),
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "No filename. Use :write <filename>",
                ));
            }
        };

        emit_hook(&HookContext::BufferWritePre {
            path: &path_owned,
            text: self.doc.slice(..),
        });

        let mut f = fs::File::create(&path_owned)?;
        for chunk in self.doc.chunks() {
            f.write_all(chunk.as_bytes())?;
        }
        self.modified = false;
        self.show_message(format!("Saved {}", path_owned.display()));

        emit_hook(&HookContext::BufferWrite { path: &path_owned });

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
            self.show_message(format!("Yanked {} chars", to - from));
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

    pub fn handle_key(&mut self, key: termina::event::KeyEvent) -> bool {
        use termina::event::KeyCode as TmKeyCode;
        use termina::event::Modifiers as TmModifiers;

        if self.scratch_open && self.scratch_focused {
            // Many terminals send Ctrl+Enter as byte 0x0A (Line Feed = Ctrl+J).
            // Termina parses this as Char('j') with CONTROL modifier.
            // We accept all three variants: Enter, '\n', and 'j' with Ctrl.
            let raw_ctrl_enter =
                matches!(key.code, TmKeyCode::Enter | TmKeyCode::Char('\n') | TmKeyCode::Char('j'))
                    && key.modifiers.contains(TmModifiers::CONTROL);

            if raw_ctrl_enter {
                return self.with_scratch_context(|ed| ed.do_execute_scratch());
            }
            return self.with_scratch_context(|ed| ed.handle_key_active(key));
        }
        self.handle_key_active(key)
    }

    fn handle_key_active(&mut self, key: termina::event::KeyEvent) -> bool {
        self.message = None;

        let old_mode = self.mode();
        let key: Key = key.into();
        let in_scratch = self.in_scratch_context;
        if self.scratch_open && self.scratch_focused {
            if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
                if matches!(self.mode(), Mode::Insert) {
                    self.input.set_mode(Mode::Normal);
                } else {
                    self.do_close_scratch();
                }
                return false;
            }
            let is_enter = matches!(key.code, KeyCode::Special(SpecialKey::Enter))
                || matches!(key.code, KeyCode::Char('\n'));
            if is_enter && (key.modifiers.ctrl || matches!(self.mode(), Mode::Normal)) {
                return self.do_execute_scratch();
            }
        }

        if in_scratch && matches!(self.mode(), Mode::Insert) && !key.modifiers.alt && !key.modifiers.ctrl {
            match key.code {
                KeyCode::Char(c) => {
                    self.insert_text(&c.to_string());
                    return false;
                }
                KeyCode::Special(SpecialKey::Enter) => {
                    self.insert_text("\n");
                    return false;
                }
                KeyCode::Special(SpecialKey::Tab) => {
                    self.insert_text("\t");
                    return false;
                }
                _ => {}
            }
        }

        let result = self.input.handle_key(key);

        match result {
            KeyResult::Action { name, count, extend, register } => {
                self.execute_action(name, count, extend, register)
            }
            KeyResult::ActionWithChar { name, count, extend, register, char_arg } => {
                self.execute_action_with_char(name, count, extend, register, char_arg)
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
            KeyResult::ExecuteCommand(cmd) => {
                self.execute_command_line(&cmd)
            }
            KeyResult::ExecuteSearch { pattern, reverse } => {
                self.input.set_last_search(pattern.clone(), reverse);
                let result = if reverse {
                    movement::find_prev(self.doc.slice(..), &pattern, self.cursor)
                } else {
                    movement::find_next(self.doc.slice(..), &pattern, self.cursor + 1)
                };
                match result {
                    Ok(Some(range)) => {
                        self.cursor = range.head;
                        self.selection = Selection::single(range.from(), range.to());
                        self.show_message(format!("Found: {}", pattern));
                    }
                    Ok(None) => {
                        self.show_message(format!("Pattern not found: {}", pattern));
                    }
                    Err(e) => {
                        self.show_error(format!("Regex error: {}", e));
                    }
                }
                false
            }
            KeyResult::SelectRegex { pattern } => {
                self.select_regex(&pattern);
                false
            }
            KeyResult::SplitRegex { pattern } => {
                self.split_regex(&pattern);
                false
            }
            KeyResult::KeepMatching { pattern } => {
                self.keep_matching(&pattern, false);
                false
            }
            KeyResult::KeepNotMatching { pattern } => {
                self.keep_matching(&pattern, true);
                false
            }
            KeyResult::PipeReplace { command } => {
                self.show_error(format!("Pipe (replace) not yet implemented: {}", command));
                false
            }
            KeyResult::PipeIgnore { command } => {
                self.show_error(format!("Pipe (ignore) not yet implemented: {}", command));
                false
            }
            KeyResult::InsertOutput { command } => {
                self.show_error(format!("Insert output not yet implemented: {}", command));
                false
            }
            KeyResult::AppendOutput { command } => {
                self.show_error(format!("Append output not yet implemented: {}", command));
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

    pub fn handle_mouse(&mut self, mouse: termina::event::MouseEvent) -> bool {
        if self.scratch_open && self.scratch_focused {
            return self.with_scratch_context(|ed| ed.handle_mouse_active(mouse));
        }
        self.handle_mouse_active(mouse)
    }

    pub fn handle_paste(&mut self, content: String) {
        if self.scratch_open && self.scratch_focused {
            self.with_scratch_context(|ed| ed.insert_text(&content));
            return;
        }

        if matches!(self.mode(), Mode::Insert) {
            self.insert_text(&content);
        } else {
            self.show_error("Paste ignored outside insert mode");
        }
    }

    pub fn handle_window_resize(&mut self, width: u16, height: u16) {
        emit_hook(&HookContext::WindowResize { width, height });
    }

    pub fn handle_focus_in(&mut self) {
        emit_hook(&HookContext::FocusGained);
    }

    pub fn handle_focus_out(&mut self) {
        emit_hook(&HookContext::FocusLost);
    }

    fn handle_mouse_active(&mut self, mouse: termina::event::MouseEvent) -> bool {
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
                    self.scroll_viewport_up();
                }
                self.move_visual_vertical(MoveDir::Backward, count, false);
            }
            ScrollDirection::Down => {
                for _ in 0..count {
                    self.scroll_viewport_down();
                }
                self.move_visual_vertical(MoveDir::Forward, count, false);
            }
            ScrollDirection::Left | ScrollDirection::Right => {
                // Horizontal scroll not implemented yet
            }
        }
    }

    fn scroll_viewport_up(&mut self) {
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

    fn scroll_viewport_down(&mut self) {
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

    fn execute_action(
        &mut self,
        name: &str,
        count: usize,
        extend: bool,
        register: Option<char>,
    ) -> bool {
        use ext::{ActionContext, ActionArgs, find_action};

        let action = match find_action(name) {

            Some(a) => a,
            None => {
                self.show_error(format!("Unknown action: {}", name));
                return false;
            }
        };

        let ctx = ActionContext {
            text: self.doc.slice(..),
            cursor: self.cursor,
            selection: &self.selection,
            count,
            extend,
            register,
            args: ActionArgs::default(),
        };

        let result = (action.handler)(&ctx);
        self.apply_action_result(result, extend)
    }

    fn execute_action_with_char(
        &mut self,
        name: &str,
        count: usize,
        extend: bool,
        register: Option<char>,
        char_arg: char,
    ) -> bool {
        use ext::{ActionContext, ActionArgs, find_action};

        let action = match find_action(name) {
            Some(a) => a,
            None => {
                self.show_error(format!("Unknown action: {}", name));
                return false;
            }
        };

        let ctx = ActionContext {
            text: self.doc.slice(..),
            cursor: self.cursor,
            selection: &self.selection,
            count,
            extend,
            register,
            args: ActionArgs {
                char: Some(char_arg),
                string: None,
            },
        };

        let result = (action.handler)(&ctx);
        self.apply_action_result(result, extend)
    }

    fn apply_action_result(&mut self, result: ext::ActionResult, extend: bool) -> bool {
        let mut ctx = ext::EditorContext::new(self);
        ext::dispatch_result(&result, &mut ctx, extend)
    }


    pub(crate) fn do_execute_edit_action(&mut self, action: ext::EditAction, _extend: bool) -> bool {

        use ext::EditAction;
        use tome_core::range::Direction as MoveDir;

        match action {
            EditAction::Delete { yank } => {
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
            EditAction::Change { yank } => {
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
                self.input.set_mode(Mode::Insert);
            }
            EditAction::Yank => {
                self.yank_selection();
            }
            EditAction::Paste { before } => {
                if before {
                    self.paste_before();
                } else {
                    self.paste_after();
                }
            }
            EditAction::PasteAll { before } => {
                if before {
                    self.paste_before();
                } else {
                    self.paste_after();
                }
            }
            EditAction::ReplaceWithChar { ch } => {
                let primary = self.selection.primary();
                let from = primary.from();
                let to = primary.to();
                if from < to {
                    self.save_undo_state();
                    let len = to - from;
                    let replacement = std::iter::repeat_n(ch, len).collect::<String>();
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    let tx = Transaction::insert(self.doc.slice(..), &self.selection, replacement);
                    tx.apply(&mut self.doc);
                    self.cursor = self.selection.primary().head + len;
                    self.selection = Selection::point(self.cursor);
                    self.modified = true;
                } else {
                    self.save_undo_state();
                    self.selection = Selection::single(from, from + 1);
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    let tx = Transaction::insert(self.doc.slice(..), &self.selection, ch.to_string());
                    tx.apply(&mut self.doc);
                    self.cursor = self.selection.primary().head + 1;
                    self.selection = Selection::point(self.cursor);
                    self.modified = true;
                }
            }
            EditAction::Undo => {
                self.undo();
            }
            EditAction::Redo => {
                self.redo();
            }
            EditAction::Indent => {
                let slice = self.doc.slice(..);
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_start(slice, *r, false);
                });
                self.insert_text("    ");
            }
            EditAction::Deindent => {
                let line = self.doc.char_to_line(self.cursor);
                let line_start = self.doc.line_to_char(line);
                let line_text: String = self.doc.line(line).chars().take(4).collect();
                let spaces = line_text.chars().take_while(|c| *c == ' ').count().min(4);
                if spaces > 0 {
                    self.save_undo_state();
                    self.selection = Selection::single(line_start, line_start + spaces);
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    self.cursor = self.cursor.saturating_sub(spaces);
                    self.modified = true;
                }
            }
            EditAction::ToLowerCase => {
                self.apply_case_conversion(|c| Box::new(c.to_lowercase()));
            }
            EditAction::ToUpperCase => {
                self.apply_case_conversion(|c| Box::new(c.to_uppercase()));
            }
            EditAction::SwapCase => {
                self.apply_case_conversion(|c| {
                    if c.is_uppercase() {
                        Box::new(c.to_lowercase())
                    } else {
                        Box::new(c.to_uppercase())
                    }
                });
            }
            EditAction::JoinLines => {
                let primary = self.selection.primary();
                let line = self.doc.char_to_line(primary.head);
                if line + 1 < self.doc.len_lines() {
                    self.save_undo_state();
                    let end_of_line = self.doc.line_to_char(line + 1) - 1;
                    self.selection = Selection::single(end_of_line, end_of_line + 1);
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    let tx = Transaction::insert(self.doc.slice(..), &self.selection, " ".to_string());
                    tx.apply(&mut self.doc);
                    self.cursor = self.selection.primary().head + 1;
                    self.selection = Selection::point(self.cursor);
                    self.modified = true;
                }
            }
            EditAction::DeleteBack => {
                if self.cursor > 0 {
                    self.save_undo_state();
                    self.selection = Selection::single(self.cursor - 1, self.cursor);
                    let tx = Transaction::delete(self.doc.slice(..), &self.selection);
                    self.selection = tx.map_selection(&self.selection);
                    tx.apply(&mut self.doc);
                    self.cursor -= 1;
                    self.modified = true;
                }
            }
            EditAction::OpenBelow => {
                let slice = self.doc.slice(..);
                self.selection.transform_mut(|r| {
                    *r = movement::move_to_line_end(slice, *r, false);
                });
                self.insert_text("\n");
                self.input.set_mode(Mode::Insert);
            }
            EditAction::OpenAbove => {
                let slice = self.doc.slice(..);
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
                self.input.set_mode(Mode::Insert);
            }
            EditAction::MoveVisual { direction, count, extend } => {
                use ext::VisualDirection;
                let dir = match direction {
                    VisualDirection::Up => MoveDir::Backward,
                    VisualDirection::Down => MoveDir::Forward,
                };
                self.move_visual_vertical(dir, count, extend);
            }
            EditAction::Scroll { direction, amount, extend: scroll_extend } => {
                use ext::{ScrollAmount, ScrollDir};
                let count = match amount {
                    ScrollAmount::Line(n) => n,
                    ScrollAmount::HalfPage => 10,
                    ScrollAmount::FullPage => 20,
                };
                let dir = match direction {
                    ScrollDir::Up => MoveDir::Backward,
                    ScrollDir::Down => MoveDir::Forward,
                };
                self.move_visual_vertical(dir, count, scroll_extend);
            }
            EditAction::AddLineBelow => {
                let current_pos = self.cursor;
                // Move cursor to line end, insert newline, then restore cursor
                let line = self.doc.char_to_line(current_pos);
                let line_end = if line + 1 < self.doc.len_lines() {
                    self.doc.line_to_char(line + 1).saturating_sub(1)
                } else {
                    self.doc.len_chars()
                };
                self.cursor = line_end;
                self.insert_text("\n");
                self.cursor = current_pos;
                self.selection = Selection::point(current_pos);
            }
            EditAction::AddLineAbove => {
                let current_pos = self.cursor;
                let line = self.doc.char_to_line(current_pos);
                let line_start = self.doc.line_to_char(line);
                self.cursor = line_start;
                self.insert_text("\n");
                self.cursor = current_pos + 1;
                self.selection = Selection::point(current_pos + 1);
            }
        }
        false
    }

    fn apply_case_conversion<F>(&mut self, char_mapper: F)
    where
        F: Fn(char) -> Box<dyn Iterator<Item = char>>,
    {
        let primary = self.selection.primary();
        let from = primary.from();
        let to = primary.to();
        if from < to {
            self.save_undo_state();
            let text: String = self
                .doc
                .slice(from..to)
                .chars()
                .flat_map(char_mapper)
                .collect();
            let new_len = text.chars().count();
            let tx = Transaction::delete(self.doc.slice(..), &self.selection);
            self.selection = tx.map_selection(&self.selection);
            tx.apply(&mut self.doc);
            let tx = Transaction::insert(self.doc.slice(..), &self.selection, text);
            tx.apply(&mut self.doc);
            self.cursor = self.selection.primary().head + new_len;
            self.selection = Selection::point(self.cursor);
            self.modified = true;
        }
    }


    pub(crate) fn do_search_next(&mut self, add_selection: bool, extend: bool) -> bool {
        if let Some((pattern, _reverse)) = self.input.last_search() {
            match movement::find_next(self.doc.slice(..), pattern, self.cursor + 1) {
                Ok(Some(range)) => {
                    self.cursor = range.head;
                    if add_selection {
                        self.selection.push(range);
                    } else if extend {
                        let anchor = self.selection.primary().anchor;
                        self.selection = Selection::single(anchor, range.to());
                    } else {
                        self.selection = Selection::single(range.from(), range.to());
                    }
                }
                Ok(None) => {
                    self.show_message("Pattern not found");
                }
                Err(e) => {
                    self.show_error(format!("Regex error: {}", e));
                }
            }
        } else {
            self.show_message("No search pattern");
        }
        false
    }

    pub(crate) fn do_search_prev(&mut self, add_selection: bool, extend: bool) -> bool {
        if let Some((pattern, _reverse)) = self.input.last_search() {
            match movement::find_prev(self.doc.slice(..), pattern, self.cursor) {
                Ok(Some(range)) => {
                    self.cursor = range.head;
                    if add_selection {
                        self.selection.push(range);
                    } else if extend {
                        let anchor = self.selection.primary().anchor;
                        self.selection = Selection::single(anchor, range.from());
                    } else {
                        self.selection = Selection::single(range.from(), range.to());
                    }
                }
                Ok(None) => {
                    self.show_message("Pattern not found");
                }
                Err(e) => {
                    self.show_error(format!("Regex error: {}", e));
                }
            }
        } else {
            self.show_message("No search pattern");
        }
        false
    }

    pub(crate) fn do_use_selection_as_search(&mut self) -> bool {
        let primary = self.selection.primary();
        let from = primary.from();
        let to = primary.to();
        if from < to {
            let text: String = self.doc.slice(from..to).chars().collect();
            let pattern = movement::escape_pattern(&text);
            self.input.set_last_search(pattern.clone(), false);
            self.show_message(format!("Search: {}", text));
            // Go to next match
            match movement::find_next(self.doc.slice(..), &pattern, to) {
                Ok(Some(range)) => {
                    self.selection = Selection::single(range.from(), range.to());
                }
                Ok(None) => {
                    self.show_message("No more matches");
                }
                Err(e) => {
                    self.show_error(format!("Regex error: {}", e));
                }
            }
        } else {
            self.show_message("No selection");
        }
        false
    }

    fn select_regex(&mut self, pattern: &str) -> bool {
        let primary = self.selection.primary();
        let from = primary.from();
        let to = primary.to();
        if from >= to {
            self.show_message("No selection to search in");
            return false;
        }

        match movement::find_all_matches(self.doc.slice(from..to), pattern) {
            Ok(matches) if !matches.is_empty() => {
                let new_ranges: Vec<tome_core::Range> = matches
                    .into_iter()
                    .map(|r| tome_core::Range::new(from + r.from(), from + r.to()))
                    .collect();
                self.selection = Selection::from_vec(new_ranges, 0);
                self.show_message(format!("{} matches", self.selection.len()));
            }
            Ok(_) => {
                self.show_message("No matches found");
            }
            Err(e) => {
                self.show_error(format!("Regex error: {}", e));
            }
        }
        false
    }

    fn split_regex(&mut self, pattern: &str) -> bool {
        let primary = self.selection.primary();
        let from = primary.from();
        let to = primary.to();
        if from >= to {
            self.show_message("No selection to split");
            return false;
        }

        match movement::find_all_matches(self.doc.slice(from..to), pattern) {
            Ok(matches) if !matches.is_empty() => {
                let mut new_ranges: Vec<tome_core::Range> = Vec::new();
                let mut last_end = from;
                for m in matches {
                    let match_start = from + m.from();
                    if match_start > last_end {
                        new_ranges.push(tome_core::Range::new(last_end, match_start));
                    }
                    last_end = from + m.to();
                }
                if last_end < to {
                    new_ranges.push(tome_core::Range::new(last_end, to));
                }
                if !new_ranges.is_empty() {
                    self.selection = Selection::from_vec(new_ranges, 0);
                    self.show_message(format!("{} splits", self.selection.len()));
                } else {
                    self.show_message("Split produced no ranges");
                }
            }
            Ok(_) => {
                self.show_message("No matches found to split on");
            }
            Err(e) => {
                self.show_error(format!("Regex error: {}", e));
            }
        }
        false
    }

    pub(crate) fn do_split_lines(&mut self) -> bool {
        let primary = self.selection.primary();
        let from = primary.from();
        let to = primary.to();
        if from >= to {
            self.show_message("No selection to split");
            return false;
        }

        let start_line = self.doc.char_to_line(from);
        let end_line = self.doc.char_to_line(to.saturating_sub(1));

        let mut new_ranges: Vec<tome_core::Range> = Vec::new();
        for line in start_line..=end_line {
            let line_start = self.doc.line_to_char(line).max(from);
            let line_end = if line + 1 < self.doc.len_lines() {
                self.doc.line_to_char(line + 1).min(to)
            } else {
                self.doc.len_chars().min(to)
            };
            if line_start < line_end {
                new_ranges.push(tome_core::Range::new(line_start, line_end));
            }
        }

        if !new_ranges.is_empty() {
            self.selection = Selection::from_vec(new_ranges, 0);
            self.show_message(format!("{} lines", self.selection.len()));
        }
        false
    }

    fn keep_matching(&mut self, pattern: &str, invert: bool) -> bool {
        let mut kept_ranges: Vec<tome_core::Range> = Vec::new();
        let mut had_error = false;
        for range in self.selection.ranges() {
            let from = range.from();
            let to = range.to();
            let text: String = self.doc.slice(from..to).chars().collect();
            match movement::matches_pattern(&text, pattern) {
                Ok(matches) => {
                    if matches != invert {
                        kept_ranges.push(*range);
                    }
                }
                Err(e) => {
                    self.show_error(format!("Regex error: {}", e));
                    had_error = true;
                    break;
                }
            }
        }

        if had_error {
            return false;
        }

        if kept_ranges.is_empty() {
            self.show_message("No selections remain");
        } else {
            let count = kept_ranges.len();
            self.selection = Selection::from_vec(kept_ranges, 0);
            self.show_message(format!("{} selections kept", count));
        }
        false
    }

    fn screen_to_doc_position(&self, screen_row: u16, screen_col: u16) -> Option<usize> {
        let total_lines = self.doc.len_lines();
        let gutter_width = self.gutter_width();

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
        self.show_message(msg);
    }

    fn error(&mut self, msg: &str) {
        self.show_error(msg);
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

    fn set_theme(&mut self, theme_name: &str) -> Result<(), String> {
        if let Some(theme) = crate::theme::get_theme(theme_name) {
            self.theme = theme;
            Ok(())
        } else {
            let mut err = format!("Theme not found: {}", theme_name);
            if let Some(suggestion) = crate::theme::suggest_theme(theme_name) {
                err.push_str(&format!(". Did you mean '{}'?", suggestion));
            }
            Err(err)
        }
    }
}
