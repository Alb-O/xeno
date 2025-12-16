pub mod types;
mod actions;
mod search;
mod navigation;

use std::fs;
use std::io::{self, Write};
use std::mem;
use std::path::PathBuf;

use tome_core::key::{KeyCode, SpecialKey};
use tome_core::range::Direction as MoveDir;
use tome_core::{
    InputHandler, Key, KeyResult, Mode, MouseEvent, Rope, Selection, Transaction,
    ext, movement,
};
use tome_core::ext::{HookContext, emit_hook};

use crate::theme::{self, Theme};

pub use types::{HistoryEntry, Message, MessageKind, Registers, ScratchState};

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
