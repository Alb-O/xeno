//! Input handling with mode stack and count prefixes.
//! New action-only path (legacy Command path removed).

use crate::ext::{find_binding, BindingMode, ObjectSelectionKind, PendingKind};
use crate::key::{Key, KeyCode, MouseButton, MouseEvent, ScrollDirection, SpecialKey};

/// Editor mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Goto,
    View,
    /// Command line input mode (for `:`, `/`, `?`, regex, pipe prompts).
    Command { prompt: char, input: String },
    /// Waiting for character input to complete an action.
    PendingAction(PendingKind),
}

/// Result of processing a key.
#[derive(Debug, Clone)]
pub enum KeyResult {
    /// An action to execute (string-based system).
    Action {
        name: &'static str,
        count: usize,
        extend: bool,
        register: Option<char>,
    },
    /// An action with a character argument (from pending completion).
    ActionWithChar {
        name: &'static str,
        count: usize,
        extend: bool,
        register: Option<char>,
        char_arg: char,
    },
    /// Mode changed (to show in status).
    ModeChange(Mode),
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
    /// Select regex matches within selection (from `s` prompt).
    SelectRegex { pattern: String },
    /// Split selection on regex (from `S` prompt).
    SplitRegex { pattern: String },
    /// Keep selections matching regex (from `alt-k` prompt).
    KeepMatching { pattern: String },
    /// Keep selections not matching regex (from `alt-K` prompt).
    KeepNotMatching { pattern: String },
    /// Pipe selection through shell command, replace with output.
    PipeReplace { command: String },
    /// Pipe selection through shell command, ignore output.
    PipeIgnore { command: String },
    /// Insert shell command output before selection.
    InsertOutput { command: String },
    /// Append shell command output after selection.
    AppendOutput { command: String },
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
    /// Last search pattern for n/N repeat.
    last_search: Option<(String, bool)>, // (pattern, reverse)
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
            last_search: None,
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
                '/' | '?' => "SEARCH",
                's' | 'S' => "SELECT",
                'k' | 'K' => "FILTER",
                '|' | '\\' | '!' | '@' => "SHELL",
                _ => "PROMPT",
            },
            Mode::PendingAction(kind) => match kind {
                PendingKind::FindChar { .. } | PendingKind::FindCharReverse { .. } => "FIND",
                PendingKind::ReplaceChar => "REPLACE",
                PendingKind::Object(_) => "OBJECT",
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

    /// Set the last search pattern (called by editor after executing search).
    pub fn set_last_search(&mut self, pattern: String, reverse: bool) {
        self.last_search = Some((pattern, reverse));
    }

    /// Get the last search pattern.
    pub fn last_search(&self) -> Option<(&str, bool)> {
        self.last_search.as_ref().map(|(p, r)| (p.as_str(), *r))
    }

    fn reset_params(&mut self) {
        self.count = 0;
        self.register = None;
        self.extend = false;
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
            Mode::PendingAction(kind) => {
                let kind = *kind;
                self.handle_pending_action_key(key, kind)
            }
        }
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) -> KeyResult {
        match event {
            MouseEvent::Press { button: MouseButton::Left, row, col, .. } => {
                KeyResult::MouseClick { row, col, extend: self.extend }
            }
            MouseEvent::Drag { button: MouseButton::Left, row, col, .. } => {
                KeyResult::MouseDrag { row, col }
            }
            MouseEvent::Scroll { direction, .. } => {
                KeyResult::MouseScroll { direction, count: 1 }
            }
            MouseEvent::Press { button: MouseButton::Right, .. }
            | MouseEvent::Press { button: MouseButton::Middle, .. }
            | MouseEvent::Drag { button: MouseButton::Right, .. }
            | MouseEvent::Drag { button: MouseButton::Middle, .. }
            | MouseEvent::Release { .. } => KeyResult::Consumed,
        }
    }

    fn handle_normal_key(&mut self, key: Key) -> KeyResult {
        if let Some(digit) = key.as_digit()
            && (digit != 0 || self.count > 0)
        {
            self.count = self.count.saturating_mul(10).saturating_add(digit);
            return KeyResult::Consumed;
        }

        if key.modifiers.shift {
            self.extend = true;
        }

        // Treat Shift as extend; drop it for key matching (Kakoune-style)
        let key = key.normalize().without_shift();

        if let Some(binding) = find_binding(BindingMode::Normal, key) {
            let count = if self.count > 0 { self.count as usize } else { 1 };
            let extend = self.extend;
            let register = self.register;
            self.reset_params();
            return KeyResult::Action {
                name: binding.action,
                count,
                extend,
                register,
            };
        }

        self.reset_params();
        KeyResult::Unhandled
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

            KeyCode::Special(SpecialKey::Backspace) => self.simple_action("delete_back"),
            KeyCode::Special(SpecialKey::Delete) => self.simple_action("delete_no_yank"),

            KeyCode::Special(SpecialKey::Left) if key.modifiers.ctrl => {
                self.simple_action("prev_word_start")
            }
            KeyCode::Special(SpecialKey::Right) if key.modifiers.ctrl => {
                self.simple_action("next_word_end")
            }
            KeyCode::Special(SpecialKey::Left) => self.simple_action("move_left"),
            KeyCode::Special(SpecialKey::Right) => self.simple_action("move_right"),
            KeyCode::Special(SpecialKey::Up) => self.simple_action("move_up_visual"),
            KeyCode::Special(SpecialKey::Down) => self.simple_action("move_down_visual"),

            KeyCode::Special(SpecialKey::Home) if key.modifiers.ctrl => {
                self.simple_action("document_start")
            }
            KeyCode::Special(SpecialKey::End) if key.modifiers.ctrl => {
                self.simple_action("document_end")
            }
            KeyCode::Special(SpecialKey::Home) => self.simple_action("move_line_start"),
            KeyCode::Special(SpecialKey::End) => self.simple_action("move_line_end"),

            KeyCode::Special(SpecialKey::PageUp) => self.simple_action("scroll_page_up"),
            KeyCode::Special(SpecialKey::PageDown) => self.simple_action("scroll_page_down"),

            _ => KeyResult::Consumed,
        }
    }

    fn handle_goto_key(&mut self, key: Key) -> KeyResult {
        if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
            self.mode = Mode::Normal;
            self.reset_params();
            return KeyResult::ModeChange(Mode::Normal);
        }

        let count = if self.count > 0 { self.count as usize } else { 1 };
        let extend = self.extend;
        let register = self.register;

        let key = key.normalize().without_shift();

        // Try new keybinding registry first
        if let Some(binding) = find_binding(BindingMode::Goto, key) {

            self.mode = Mode::Normal;
            self.reset_params();
            return KeyResult::Action {
                name: binding.action,
                count,
                extend,
                register,
            };
        }

        self.mode = Mode::Normal;
        self.reset_params();
        KeyResult::Unhandled
    }

    fn handle_view_key(&mut self, key: Key) -> KeyResult {
        if matches!(key.code, KeyCode::Special(SpecialKey::Escape)) {
            self.mode = Mode::Normal;
            self.reset_params();
            return KeyResult::ModeChange(Mode::Normal);
        }

        let count = if self.count > 0 { self.count as usize } else { 1 };
        let extend = self.extend;
        let register = self.register;

        let key = key.normalize().without_shift();

        // Try new keybinding registry first
        if let Some(binding) = find_binding(BindingMode::View, key) {

            self.mode = Mode::Normal;
            self.reset_params();
            return KeyResult::Action {
                name: binding.action,
                count,
                extend,
                register,
            };
        }

        self.mode = Mode::Normal;
        self.reset_params();
        KeyResult::Unhandled
    }

    fn handle_command_key(&mut self, key: Key, prompt: char, mut input: String) -> KeyResult {
        match key.code {
            KeyCode::Special(SpecialKey::Escape) => {
                self.mode = Mode::Normal;
                self.reset_params();
                KeyResult::ModeChange(Mode::Normal)
            }

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
                    's' => KeyResult::SelectRegex { pattern: input },
                    'S' => KeyResult::SplitRegex { pattern: input },
                    'k' => KeyResult::KeepMatching { pattern: input },
                    'K' => KeyResult::KeepNotMatching { pattern: input },
                    '|' => KeyResult::PipeReplace { command: input },
                    '\\' => KeyResult::PipeIgnore { command: input },
                    '!' => KeyResult::InsertOutput { command: input },
                    '@' => KeyResult::AppendOutput { command: input },
                    _ => KeyResult::Consumed,
                }
            }

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

            KeyCode::Char(c) if key.modifiers.is_empty() => {
                input.push(c);
                self.mode = Mode::Command { prompt, input };
                KeyResult::Consumed
            }

            KeyCode::Char(' ') => {
                input.push(' ');
                self.mode = Mode::Command { prompt, input };
                KeyResult::Consumed
            }

            _ => KeyResult::Consumed,
        }
    }

    fn handle_pending_action_key(&mut self, key: Key, pending: PendingKind) -> KeyResult {
        match pending {
            PendingKind::FindChar { inclusive: _ } => match key.code {
                KeyCode::Char(ch) => {
                    let count = self.effective_count() as usize;
                    let extend = self.extend;
                    let register = self.register;
                    self.reset_params();
                    KeyResult::ActionWithChar {
                        name: "find_char",
                        count,
                        extend,
                        register,
                        char_arg: ch,
                    }
                }
                KeyCode::Special(SpecialKey::Escape) => {
                    self.mode = Mode::Normal;
                    self.reset_params();
                    KeyResult::ModeChange(Mode::Normal)
                }
                _ => KeyResult::Consumed,
            },

            PendingKind::FindCharReverse { inclusive: _ } => match key.code {
                KeyCode::Char(ch) => {
                    let count = self.effective_count() as usize;
                    let extend = self.extend;
                    let register = self.register;
                    self.reset_params();
                    KeyResult::ActionWithChar {
                        name: "find_char_reverse",
                        count,
                        extend,
                        register,
                        char_arg: ch,
                    }
                }
                KeyCode::Special(SpecialKey::Escape) => {
                    self.mode = Mode::Normal;
                    self.reset_params();
                    KeyResult::ModeChange(Mode::Normal)
                }
                _ => KeyResult::Consumed,
            },

            PendingKind::ReplaceChar => match key.code {
                KeyCode::Char(ch) => {
                    let count = self.effective_count() as usize;
                    let extend = self.extend;
                    let register = self.register;
                    self.reset_params();
                    KeyResult::ActionWithChar {
                        name: "replace_char",
                        count,
                        extend,
                        register,
                        char_arg: ch,
                    }
                }
                KeyCode::Special(SpecialKey::Escape) => {
                    self.mode = Mode::Normal;
                    self.reset_params();
                    KeyResult::ModeChange(Mode::Normal)
                }
                _ => KeyResult::Consumed,
            },

            PendingKind::Object(selection) => match key.code {
                KeyCode::Char(ch) => {
                    let count = self.effective_count() as usize;
                    let extend = self.extend;
                    let register = self.register;
                    self.reset_params();

                    let kind = match selection {
                        ObjectSelectionKind::Inner => Some("select_object_inner"),
                        ObjectSelectionKind::Around => Some("select_object_around"),
                        ObjectSelectionKind::ToStart => Some("select_object_to_start"),
                        ObjectSelectionKind::ToEnd => Some("select_object_to_end"),
                    };

                    match kind {
                        Some(action) => KeyResult::ActionWithChar {
                            name: action,
                            count,
                            extend,
                            register,
                            char_arg: ch,
                        },
                        None => KeyResult::Consumed,
                    }
                }
                KeyCode::Special(SpecialKey::Escape) => {
                    self.mode = Mode::Normal;
                    self.reset_params();
                    KeyResult::ModeChange(Mode::Normal)
                }
                _ => KeyResult::Consumed,
            },
        }
    }

    fn simple_action(&mut self, name: &'static str) -> KeyResult {
        let count = if self.count > 0 { self.count as usize } else { 1 };
        let extend = self.extend;
        let register = self.register;
        self.reset_params();
        KeyResult::Action {
            name,
            count,
            extend,
            register,
        }
    }
}

// Tests for the action-only input handler are intentionally minimal for now.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digit_count_accumulates() {
        let mut h = InputHandler::new();
        h.handle_key(Key::char('2'));
        h.handle_key(Key::char('3'));
        assert_eq!(h.effective_count(), 23);
    }
}
