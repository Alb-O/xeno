//! Input handling with mode stack and count prefixes.
//!
//! This module provides the InputHandler that processes key input,
//! manages the mode stack (Normal, Insert, Goto, View, etc.),
//! and handles count prefixes like Kakoune.

use crate::key::{Key, KeyCode, MouseButton, MouseEvent, ScrollDirection, SpecialKey};
use crate::keymap::{
    lookup, Command, CommandParams, Mode, ObjectSelection, PendingCommand, GOTO_KEYMAP,
    NORMAL_KEYMAP, VIEW_KEYMAP,
};

/// Result of processing a key.
#[derive(Debug, Clone)]
pub enum KeyResult {
    /// A command to execute.
    Command(Command, CommandParams),
    /// Mode changed (to show in status).
    ModeChange(Mode),
    /// Waiting for more input.
    Pending(String),
    /// Key was consumed but no action needed.
    Consumed,
    /// Key was not handled.
    Unhandled,
    /// Insert a character (in insert mode).
    InsertChar(char),
    /// Execute a command-line command (from `:` prompt).
    ExecuteCommand(String),
    /// Execute a search (from `/` or `?` prompt).
    ExecuteSearch { pattern: String, reverse: bool },
    /// Request to quit.
    Quit,
    /// Mouse click at screen coordinates.
    MouseClick {
        row: u16,
        col: u16,
        extend: bool,
    },
    /// Mouse drag to screen coordinates (extend selection).
    MouseDrag {
        row: u16,
        col: u16,
    },
    /// Mouse scroll.
    MouseScroll {
        direction: ScrollDirection,
        count: usize,
    },
}

/// Manages input state and key processing.
#[derive(Debug, Clone)]
pub struct InputHandler {
    mode: Mode,
    count: u32,
    register: Option<char>,
    /// For commands that extend selection.
    extend: bool,
    /// Last find command for repeat.
    last_find: Option<(char, bool, bool)>, // (char, inclusive, reverse)
    /// Last object selection for repeat.
    last_object: Option<(ObjectSelection, char)>,
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            count: 0,
            register: None,
            extend: false,
            last_find: None,
            last_object: None,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode.clone()
    }

    pub fn mode_name(&self) -> &'static str {
        match &self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Goto => "GOTO",
            Mode::View => "VIEW",
            Mode::Command { prompt, .. } => match prompt {
                ':' => "COMMAND",
                '/' => "SEARCH",
                '?' => "SEARCH",
                _ => "PROMPT",
            },
            Mode::Pending(p) => match p {
                PendingCommand::FindChar { .. } | PendingCommand::FindCharReverse { .. } => "FIND",
                PendingCommand::Replace => "REPLACE",
                PendingCommand::Register => "REG",
                PendingCommand::Object(_) => "OBJECT",
            },
        }
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn effective_count(&self) -> u32 {
        if self.count == 0 { 1 } else { self.count }
    }

    pub fn register(&self) -> Option<char> {
        self.register
    }

    /// Get command line input if in command mode.
    pub fn command_line(&self) -> Option<(char, &str)> {
        match &self.mode {
            Mode::Command { prompt, input } => Some((*prompt, input.as_str())),
            _ => None,
        }
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode.clone();
        if matches!(mode, Mode::Normal) {
            self.reset_params();
        }
    }

    fn reset_params(&mut self) {
        self.count = 0;
        self.register = None;
        self.extend = false;
    }

    fn make_params(&self) -> CommandParams {
        CommandParams {
            count: self.effective_count(),
            register: self.register,
            extend: self.extend,
        }
    }

    /// Process a key and return the result.
    pub fn handle_key(&mut self, key: Key) -> KeyResult {
        match &self.mode {
            Mode::Normal => self.handle_normal_key(key),
            Mode::Insert => self.handle_insert_key(key),
            Mode::Goto => self.handle_goto_key(key),
            Mode::View => self.handle_view_key(key),
            Mode::Command { prompt, input } => {
                let prompt = *prompt;
                let input = input.clone();
                self.handle_command_key(key, prompt, input)
            }
            Mode::Pending(pending) => {
                let pending = *pending;
                self.handle_pending_key(key, pending)
            }
        }
    }

    /// Process a mouse event and return the result.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> KeyResult {
        match event {
            MouseEvent::Press { button: MouseButton::Left, row, col, modifiers } => {
                KeyResult::MouseClick {
                    row,
                    col,
                    extend: modifiers.shift,
                }
            }
            MouseEvent::Drag { button: MouseButton::Left, row, col, .. } => {
                KeyResult::MouseDrag { row, col }
            }
            MouseEvent::Scroll { direction, .. } => {
                let count = 3; // scroll 3 lines at a time
                KeyResult::MouseScroll { direction, count }
            }
            MouseEvent::Press { button: MouseButton::Right, .. }
            | MouseEvent::Press { button: MouseButton::Middle, .. }
            | MouseEvent::Drag { button: MouseButton::Right, .. }
            | MouseEvent::Drag { button: MouseButton::Middle, .. }
            | MouseEvent::Release { .. } => KeyResult::Consumed,
        }
    }

    fn handle_normal_key(&mut self, key: Key) -> KeyResult {
        // Handle count prefix (digits)
        if let Some(digit) = key.as_digit() {
            // '0' is only a count if we already have a count
            if digit != 0 || self.count > 0 {
                self.count = self.count.saturating_mul(10).saturating_add(digit);
                return KeyResult::Consumed;
            }
        }

        // Handle register prefix (")
        if key.is_char('"') && self.register.is_none() {
            self.mode = Mode::Pending(PendingCommand::Register);
            return KeyResult::Pending("register...".into());
        }

        // Check if shift is held for extend mode
        if key.modifiers.shift {
            self.extend = true;
        }

        // Look up in keymap
        if let Some(mapping) = lookup(NORMAL_KEYMAP, key) {
            self.process_command(mapping.command)
        } else {
            self.reset_params();
            KeyResult::Unhandled
        }
    }

    fn process_command(&mut self, command: Command) -> KeyResult {
        let params = self.make_params();

        match command {
            // Mode changes
            Command::InsertBefore
            | Command::InsertAfter
            | Command::InsertLineStart
            | Command::InsertLineEnd
            | Command::OpenBelow
            | Command::OpenAbove => {
                self.reset_params();
                self.mode = Mode::Insert;
                KeyResult::Command(command, params)
            }

            Command::Change { yank } => {
                self.reset_params();
                self.mode = Mode::Insert;
                KeyResult::Command(Command::Change { yank }, params)
            }

            Command::EnterGotoMode => {
                // If count is given, go to line number directly
                if params.count > 1 || self.count > 0 {
                    self.reset_params();
                    return KeyResult::Command(Command::MoveDocumentStart, params);
                }
                self.mode = Mode::Goto;
                // Note: self.extend is preserved for G (extend goto mode)
                KeyResult::ModeChange(Mode::Goto)
            }

            Command::EnterViewMode => {
                self.mode = Mode::View;
                KeyResult::ModeChange(Mode::View)
            }

            Command::EnterCommandMode => {
                self.mode = Mode::Command {
                    prompt: ':',
                    input: String::new(),
                };
                KeyResult::ModeChange(self.mode.clone())
            }

            Command::SearchForward => {
                self.mode = Mode::Command {
                    prompt: '/',
                    input: String::new(),
                };
                KeyResult::ModeChange(self.mode.clone())
            }

            Command::SearchBackward => {
                self.mode = Mode::Command {
                    prompt: '?',
                    input: String::new(),
                };
                KeyResult::ModeChange(self.mode.clone())
            }

            Command::FindCharForward { inclusive, ch } => {
                if let Some(c) = ch {
                    // Already have a char (from repeat)
                    self.last_find = Some((c, inclusive, false));
                    self.reset_params();
                    return KeyResult::Command(
                        Command::FindCharForward { inclusive, ch: Some(c) },
                        params,
                    );
                }
                self.mode = Mode::Pending(PendingCommand::FindChar {
                    inclusive,
                    extend: self.extend,
                });
                KeyResult::Pending("find→".into())
            }

            Command::FindCharBackward { inclusive, ch } => {
                if let Some(c) = ch {
                    // Already have a char (from repeat)
                    self.last_find = Some((c, inclusive, true));
                    self.reset_params();
                    return KeyResult::Command(
                        Command::FindCharBackward { inclusive, ch: Some(c) },
                        params,
                    );
                }
                self.mode = Mode::Pending(PendingCommand::FindCharReverse {
                    inclusive,
                    extend: self.extend,
                });
                KeyResult::Pending("find←".into())
            }

            Command::ReplaceWithChar => {
                self.mode = Mode::Pending(PendingCommand::Replace);
                KeyResult::Pending("replace".into())
            }

            Command::SelectObject { trigger, selection } => {
                if let Some(ch) = trigger {
                    self.last_object = Some((selection, ch));
                    let params = self.make_params();
                    self.reset_params();
                    return KeyResult::Command(
                        Command::SelectObject { trigger: Some(ch), selection },
                        params,
                    );
                }
                let prompt = match selection {
                    ObjectSelection::Inner => "inner",
                    ObjectSelection::Around => "around",
                    ObjectSelection::ToStart => "[obj",
                    ObjectSelection::ToEnd => "]obj",
                };
                self.mode = Mode::Pending(PendingCommand::Object(selection));
                KeyResult::Pending(prompt.into())
            }

            Command::RepeatLastFind => {
                if let Some((ch, inclusive, reverse)) = self.last_find {
                    self.reset_params();
                    return KeyResult::Command(
                        if reverse {
                            Command::FindCharBackward { inclusive, ch: Some(ch) }
                        } else {
                            Command::FindCharForward { inclusive, ch: Some(ch) }
                        },
                        CommandParams {
                            count: params.count,
                            register: params.register,
                            extend: params.extend,
                        },
                    );
                }
                self.reset_params();
                KeyResult::Consumed
            }

            Command::RepeatLastFindReverse => {
                if let Some((ch, inclusive, reverse)) = self.last_find {
                    self.reset_params();
                    return KeyResult::Command(
                        if reverse {
                            Command::FindCharForward { inclusive, ch: Some(ch) }
                        } else {
                            Command::FindCharBackward { inclusive, ch: Some(ch) }
                        },
                        CommandParams {
                            count: params.count,
                            register: params.register,
                            extend: params.extend,
                        },
                    );
                }
                self.reset_params();
                KeyResult::Consumed
            }

            Command::Escape => {
                self.reset_params();
                KeyResult::Command(Command::Escape, params)
            }

            Command::Quit | Command::QuitForce => {
                self.reset_params();
                KeyResult::Quit
            }

            _ => {
                self.reset_params();
                KeyResult::Command(command, params)
            }
        }
    }

    fn handle_insert_key(&mut self, key: Key) -> KeyResult {
        match key.code {
            KeyCode::Special(SpecialKey::Escape) => {
                self.mode = Mode::Normal;
                self.reset_params();
                KeyResult::ModeChange(Mode::Normal)
            }

            KeyCode::Char(c) if key.modifiers.is_empty() => KeyResult::InsertChar(c),

            KeyCode::Special(SpecialKey::Enter) => KeyResult::InsertChar('\n'),

            KeyCode::Special(SpecialKey::Tab) => KeyResult::InsertChar('\t'),

            KeyCode::Special(SpecialKey::Backspace) => {
                KeyResult::Command(Command::DeleteBack, CommandParams::default())
            }

            KeyCode::Special(SpecialKey::Delete) => {
                KeyResult::Command(Command::Delete { yank: false }, CommandParams::default())
            }

            KeyCode::Special(SpecialKey::Left) => {
                KeyResult::Command(Command::MoveLeft, CommandParams::default())
            }

            KeyCode::Special(SpecialKey::Right) => {
                KeyResult::Command(Command::MoveRight, CommandParams::default())
            }

            KeyCode::Special(SpecialKey::Up) => {
                KeyResult::Command(Command::MoveUp, CommandParams::default())
            }

            KeyCode::Special(SpecialKey::Down) => {
                KeyResult::Command(Command::MoveDown, CommandParams::default())
            }

            // Ctrl+r for register insert
            KeyCode::Char('r') if key.modifiers.ctrl => {
                self.mode = Mode::Pending(PendingCommand::Register);
                KeyResult::Pending("reg".into())
            }

            // Alt+; to escape for one normal command
            KeyCode::Char(';') if key.modifiers.alt => {
                // TODO: implement single-command escape
                KeyResult::Consumed
            }

            _ => KeyResult::Consumed,
        }
    }

    fn handle_goto_key(&mut self, key: Key) -> KeyResult {
        // Escape returns to normal
        if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
            self.mode = Mode::Normal;
            self.reset_params();
            return KeyResult::ModeChange(Mode::Normal);
        }

        let params = self.make_params();

        if let Some(mapping) = lookup(GOTO_KEYMAP, key) {
            self.mode = Mode::Normal;
            self.reset_params();
            KeyResult::Command(mapping.command, params)
        } else {
            self.mode = Mode::Normal;
            self.reset_params();
            KeyResult::Unhandled
        }
    }

    fn handle_view_key(&mut self, key: Key) -> KeyResult {
        // Escape returns to normal
        if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
            self.mode = Mode::Normal;
            self.reset_params();
            return KeyResult::ModeChange(Mode::Normal);
        }

        let params = self.make_params();

        if let Some(mapping) = lookup(VIEW_KEYMAP, key) {
            // View mode stays in view mode (locked) unless escape
            // For non-locked, we'd return to normal
            self.mode = Mode::Normal;
            self.reset_params();
            KeyResult::Command(mapping.command, params)
        } else {
            self.mode = Mode::Normal;
            self.reset_params();
            KeyResult::Unhandled
        }
    }

    fn handle_command_key(&mut self, key: Key, prompt: char, mut input: String) -> KeyResult {
        match key.code {
            // Escape cancels command mode
            KeyCode::Special(SpecialKey::Escape) => {
                self.mode = Mode::Normal;
                self.reset_params();
                KeyResult::ModeChange(Mode::Normal)
            }

            // Enter executes the command
            KeyCode::Special(SpecialKey::Enter) => {
                self.mode = Mode::Normal;
                self.reset_params();

                if input.is_empty() {
                    return KeyResult::Consumed;
                }

                match prompt {
                    ':' => KeyResult::ExecuteCommand(input),
                    '/' => KeyResult::ExecuteSearch {
                        pattern: input,
                        reverse: false,
                    },
                    '?' => KeyResult::ExecuteSearch {
                        pattern: input,
                        reverse: true,
                    },
                    _ => KeyResult::Consumed,
                }
            }

            // Backspace deletes last char
            KeyCode::Special(SpecialKey::Backspace) => {
                if input.is_empty() {
                    self.mode = Mode::Normal;
                    self.reset_params();
                    KeyResult::ModeChange(Mode::Normal)
                } else {
                    input.pop();
                    self.mode = Mode::Command { prompt, input };
                    KeyResult::Consumed
                }
            }

            // Regular character adds to input
            KeyCode::Char(c) if key.modifiers.is_empty() => {
                input.push(c);
                self.mode = Mode::Command { prompt, input };
                KeyResult::Consumed
            }

            // Space adds to input
            KeyCode::Char(' ') => {
                input.push(' ');
                self.mode = Mode::Command { prompt, input };
                KeyResult::Consumed
            }

            _ => KeyResult::Consumed,
        }
    }

    fn handle_pending_key(&mut self, key: Key, pending: PendingCommand) -> KeyResult {
        // Escape cancels pending
        if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
            self.mode = Mode::Normal;
            self.reset_params();
            return KeyResult::ModeChange(Mode::Normal);
        }

        match pending {
            PendingCommand::Register => {
                if let Some(c) = key.codepoint() {
                    self.register = Some(c);
                    self.mode = Mode::Normal;
                    return KeyResult::Consumed;
                }
                self.mode = Mode::Normal;
                self.reset_params();
                KeyResult::Unhandled
            }

            PendingCommand::FindChar { inclusive, extend } => {
                if let Some(c) = key.codepoint() {
                    self.last_find = Some((c, inclusive, false));
                    let params = CommandParams {
                        count: self.effective_count(),
                        register: self.register,
                        extend,
                    };
                    self.mode = Mode::Normal;
                    self.reset_params();
                    return KeyResult::Command(
                        Command::FindCharForward { inclusive, ch: Some(c) },
                        params,
                    );
                }
                self.mode = Mode::Normal;
                self.reset_params();
                KeyResult::Unhandled
            }

            PendingCommand::FindCharReverse { inclusive, extend } => {
                if let Some(c) = key.codepoint() {
                    self.last_find = Some((c, inclusive, true));
                    let params = CommandParams {
                        count: self.effective_count(),
                        register: self.register,
                        extend,
                    };
                    self.mode = Mode::Normal;
                    self.reset_params();
                    return KeyResult::Command(
                        Command::FindCharBackward { inclusive, ch: Some(c) },
                        params,
                    );
                }
                self.mode = Mode::Normal;
                self.reset_params();
                KeyResult::Unhandled
            }

            PendingCommand::Replace => {
                if let Some(_) = key.codepoint() {
                    let params = self.make_params();
                    self.mode = Mode::Normal;
                    self.reset_params();
                    return KeyResult::Command(Command::Replace, params);
                }
                self.mode = Mode::Normal;
                self.reset_params();
                KeyResult::Unhandled
            }

            PendingCommand::Object(selection) => {
                if let Some(c) = key.codepoint() {
                    // Any char can be a trigger - the ext registry will validate
                    self.last_object = Some((selection, c));
                    let params = self.make_params();
                    self.mode = Mode::Normal;
                    self.reset_params();
                    return KeyResult::Command(
                        Command::SelectObject {
                            trigger: Some(c),
                            selection,
                        },
                        params,
                    );
                }
                self.mode = Mode::Normal;
                self.reset_params();
                KeyResult::Unhandled
            }
        }
    }

    /// Get status info for display.
    pub fn status(&self) -> String {
        let mut parts = Vec::new();

        if self.count > 0 {
            parts.push(self.count.to_string());
        }

        if let Some(reg) = self.register {
            parts.push(format!("\"{}\"", reg));
        }

        parts.push(self.mode_name().to_string());

        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_accumulation() {
        let mut handler = InputHandler::new();

        handler.handle_key(Key::char('3'));
        assert_eq!(handler.count, 3);

        handler.handle_key(Key::char('5'));
        assert_eq!(handler.count, 35);
    }

    #[test]
    fn test_movement_command() {
        let mut handler = InputHandler::new();

        let result = handler.handle_key(Key::char('h'));
        assert!(matches!(
            result,
            KeyResult::Command(Command::MoveLeft, _)
        ));
    }

    #[test]
    fn test_count_with_command() {
        let mut handler = InputHandler::new();

        handler.handle_key(Key::char('3'));
        let result = handler.handle_key(Key::char('w'));

        if let KeyResult::Command(Command::MoveNextWordStart, params) = result {
            assert_eq!(params.count, 3);
        } else {
            panic!("Expected MoveNextWordStart command");
        }
    }

    #[test]
    fn test_insert_mode_entry() {
        let mut handler = InputHandler::new();

        let result = handler.handle_key(Key::char('i'));
        assert!(matches!(
            result,
            KeyResult::Command(Command::InsertBefore, _)
        ));
        assert_eq!(handler.mode, Mode::Insert);
    }

    #[test]
    fn test_insert_mode_escape() {
        let mut handler = InputHandler::new();
        handler.mode = Mode::Insert;

        let result = handler.handle_key(Key::special(SpecialKey::Escape));
        assert!(matches!(result, KeyResult::ModeChange(Mode::Normal)));
        assert_eq!(handler.mode, Mode::Normal);
    }

    #[test]
    fn test_goto_mode() {
        let mut handler = InputHandler::new();

        let result = handler.handle_key(Key::char('g'));
        assert!(matches!(result, KeyResult::ModeChange(Mode::Goto)));

        let result = handler.handle_key(Key::char('g'));
        assert!(matches!(
            result,
            KeyResult::Command(Command::MoveDocumentStart, _)
        ));
        assert_eq!(handler.mode, Mode::Normal);
    }

    #[test]
    fn test_insert_char() {
        let mut handler = InputHandler::new();
        handler.mode = Mode::Insert;

        let result = handler.handle_key(Key::char('a'));
        assert!(matches!(result, KeyResult::InsertChar('a')));
    }

    #[test]
    fn test_insert_mode_arrow_keys() {
        let mut handler = InputHandler::new();
        handler.mode = Mode::Insert;

        let result = handler.handle_key(Key::special(SpecialKey::Left));
        assert!(matches!(result, KeyResult::Command(Command::MoveLeft, _)));

        let result = handler.handle_key(Key::special(SpecialKey::Right));
        assert!(matches!(result, KeyResult::Command(Command::MoveRight, _)));

        let result = handler.handle_key(Key::special(SpecialKey::Up));
        assert!(matches!(result, KeyResult::Command(Command::MoveUp, _)));

        let result = handler.handle_key(Key::special(SpecialKey::Down));
        assert!(matches!(result, KeyResult::Command(Command::MoveDown, _)));

        // Should still be in insert mode
        assert!(matches!(handler.mode, Mode::Insert));
    }

    #[test]
    fn test_status() {
        let mut handler = InputHandler::new();
        assert_eq!(handler.status(), "NORMAL");

        handler.handle_key(Key::char('3'));
        assert_eq!(handler.status(), "3 NORMAL");
    }

    #[test]
    fn test_quit_command() {
        let mut handler = InputHandler::new();

        let result = handler.handle_key(Key::ctrl('q'));
        assert!(matches!(result, KeyResult::Quit));
    }

    #[test]
    fn test_command_mode() {
        let mut handler = InputHandler::new();

        // Enter command mode with ':'
        let result = handler.handle_key(Key::char(':'));
        assert!(matches!(result, KeyResult::ModeChange(Mode::Command { .. })));
        assert!(matches!(handler.mode(), Mode::Command { prompt: ':', .. }));

        // Type 'q'
        handler.handle_key(Key::char('q'));
        assert_eq!(handler.command_line(), Some((':', "q")));

        // Press Enter to execute
        let result = handler.handle_key(Key::special(SpecialKey::Enter));
        assert!(matches!(result, KeyResult::ExecuteCommand(ref s) if s == "q"));
        assert!(matches!(handler.mode(), Mode::Normal));
    }

    #[test]
    fn test_command_mode_escape() {
        let mut handler = InputHandler::new();

        handler.handle_key(Key::char(':'));
        handler.handle_key(Key::char('w'));
        handler.handle_key(Key::char('q'));

        // Escape cancels command mode
        let result = handler.handle_key(Key::special(SpecialKey::Escape));
        assert!(matches!(result, KeyResult::ModeChange(Mode::Normal)));
        assert!(matches!(handler.mode(), Mode::Normal));
    }

    #[test]
    fn test_find_char_repeat() {
        let mut handler = InputHandler::new();

        // 'f' enters pending mode for find char forward
        let result = handler.handle_key(Key::char('f'));
        assert!(matches!(result, KeyResult::Pending(_)));

        // Type 'x' to find
        let result = handler.handle_key(Key::char('x'));
        assert!(matches!(
            result,
            KeyResult::Command(Command::FindCharForward { inclusive: true, ch: Some('x') }, _)
        ));

        // alt-. repeats the find
        let result = handler.handle_key(Key::alt('.'));
        assert!(matches!(
            result,
            KeyResult::Command(Command::FindCharForward { inclusive: true, ch: Some('x') }, _)
        ));

        // Now do a backward find with alt-f
        handler.handle_key(Key::alt('f'));
        let result = handler.handle_key(Key::char('y'));
        assert!(matches!(
            result,
            KeyResult::Command(Command::FindCharBackward { inclusive: true, ch: Some('y') }, _)
        ));

        // alt-. now repeats backward find
        let result = handler.handle_key(Key::alt('.'));
        assert!(matches!(
            result,
            KeyResult::Command(Command::FindCharBackward { inclusive: true, ch: Some('y') }, _)
        ));
    }
}
