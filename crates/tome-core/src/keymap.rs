//! Keymap definitions for Kakoune-compatible keybindings.
//!
//! This module defines the mapping from keys to editor commands,
//! following Kakoune's design principles:
//! - lowercase = replace selection
//! - UPPERCASE = extend selection
//! - alt+key = alternative/reverse direction
//! - ctrl+key = special commands

use crate::key::{Key, SpecialKey};

/// Selection mode for movement commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    /// Replace the selection (move cursor, anchor follows).
    Replace,
    /// Extend the selection (move cursor, anchor stays).
    Extend,
}

/// The editing mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Goto,
    View,
    /// Command line input mode (for `:`, `/`, `?`).
    Command { prompt: char, input: String },
    /// Waiting for next character (e.g., for `f`, `t`, `r` commands).
    Pending(PendingCommand),
}

/// Commands that wait for a character argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingCommand {
    FindChar { inclusive: bool, extend: bool },
    FindCharReverse { inclusive: bool, extend: bool },
    Replace,
    /// Waiting for register name.
    Register,
    /// Waiting for object type (inner/around).
    Object(ObjectFlags),
}

/// Flags for object selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectFlags {
    pub to_begin: bool,
    pub to_end: bool,
    pub inner: bool,
    pub extend: bool,
}

impl ObjectFlags {
    pub const INNER: Self = Self {
        to_begin: true,
        to_end: true,
        inner: true,
        extend: false,
    };

    pub const AROUND: Self = Self {
        to_begin: true,
        to_end: true,
        inner: false,
        extend: false,
    };

    pub const TO_BEGIN: Self = Self {
        to_begin: true,
        to_end: false,
        inner: false,
        extend: false,
    };

    pub const TO_END: Self = Self {
        to_begin: false,
        to_end: true,
        inner: false,
        extend: false,
    };

    pub fn with_extend(self) -> Self {
        Self {
            extend: true,
            ..self
        }
    }

    pub fn with_inner(self) -> Self {
        Self {
            inner: true,
            ..self
        }
    }
}

/// A command that the editor can execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    // Movement (basic)
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,

    // Movement (word)
    MoveNextWordStart,
    MoveNextWordEnd,
    MovePrevWordStart,
    MoveNextWORDStart,
    MoveNextWORDEnd,
    MovePrevWORDStart,

    // Movement (line)
    MoveLineStart,
    MoveLineEnd,
    MoveFirstNonWhitespace,

    // Movement (document)
    MoveDocumentStart,
    MoveDocumentEnd,

    // Find character
    FindCharForward { inclusive: bool, ch: Option<char> },
    FindCharBackward { inclusive: bool, ch: Option<char> },
    RepeatLastFind,
    RepeatLastFindReverse,

    // Selection manipulation
    CollapseSelection,
    FlipSelection,
    EnsureForward,
    KeepPrimarySelection,
    RemovePrimarySelection,
    RotateSelectionsForward,
    RotateSelectionsBackward,

    // Expand selections
    SelectLine,
    TrimToLine,
    SelectAll,

    // Changes
    Delete { yank: bool },
    Change { yank: bool },
    Yank,
    Paste { before: bool },
    PasteAll { before: bool },
    Replace,
    ReplaceWithYanked,
    ReplaceWithChar,

    // Insert mode entry
    InsertBefore,
    InsertAfter,
    InsertLineStart,
    InsertLineEnd,
    OpenBelow,
    OpenAbove,

    // Undo/Redo
    Undo,
    Redo,
    UndoSelectionChange,
    RedoSelectionChange,

    // Search
    SearchForward,
    SearchBackward,
    SearchNext,
    SearchPrev,
    SearchNextAdd,
    SearchPrevAdd,
    UseSelectionAsSearch,

    // Macros
    RecordMacro,
    PlayMacro,

    // Marks/Registers
    SaveSelections,
    RestoreSelections,

    // Indent
    Indent,
    Deindent,

    // Case
    ToLowerCase,
    ToUpperCase,
    SwapCase,

    // Join
    JoinLines,
    JoinLinesSelect,

    // Object selection - these enter pending mode when object_type is None
    SelectInnerObject { object_type: Option<ObjectType> },
    SelectAroundObject { object_type: Option<ObjectType> },
    SelectToObjectStart { object_type: Option<ObjectType> },
    SelectToObjectEnd { object_type: Option<ObjectType> },
    ExtendToObjectStart { object_type: Option<ObjectType> },
    ExtendToObjectEnd { object_type: Option<ObjectType> },

    // Scrolling
    ScrollUp,
    ScrollDown,
    ScrollHalfPageUp,
    ScrollHalfPageDown,
    ScrollPageUp,
    ScrollPageDown,

    // Jump list
    JumpForward,
    JumpBackward,
    PushJump,

    // Regex selection
    SelectRegex,
    SplitRegex,
    SplitLines,
    KeepMatching,
    KeepNotMatching,

    // View commands (sub-mode)
    EnterViewMode,

    // Goto commands (sub-mode)
    EnterGotoMode,

    // Command prompt
    EnterCommandMode,

    // Pipe/shell
    PipeReplace,
    PipeIgnore,
    InsertOutput,
    AppendOutput,

    // Repeat
    RepeatLastInsert,
    RepeatLastObject,

    // Misc
    ForceRedraw,
    AddEmptyLineBelow,
    AddEmptyLineAbove,
    DuplicateSelections,
    MergeSelections,
    Align,
    CopyIndent,
    TabsToSpaces,
    SpacesToTabs,
    TrimSelections,
    DuplicateSelectionsOnNextLines,
    DuplicateSelectionsOnPrevLines,

    // User/Space menu
    UserMappings,

    // Escape/Cancel
    Escape,

    // Quit
    Quit,
    QuitForce,

    // No-op for unmapped keys
    None,
}

/// Parameters passed to commands.
#[derive(Debug, Clone, Copy, Default)]
pub struct CommandParams {
    /// Numeric count prefix (0 means no count).
    pub count: u32,
    /// Register to use.
    pub register: Option<char>,
    /// Whether to extend selection.
    pub extend: bool,
}

/// Describes a command mapping.
#[derive(Debug, Clone, Copy)]
pub struct KeyMapping {
    pub key: Key,
    pub command: Command,
    pub doc: &'static str,
}

impl KeyMapping {
    const fn new(key: Key, command: Command, doc: &'static str) -> Self {
        Self { key, command, doc }
    }
}

/// Helper macro to make keymaps more readable.
macro_rules! key {
    ($c:literal) => {
        Key::char($c)
    };
    (ctrl $c:literal) => {
        Key::ctrl($c)
    };
    (alt $c:literal) => {
        Key::alt($c)
    };
    (special $s:ident) => {
        Key::special(SpecialKey::$s)
    };
}

/// The complete normal mode keymap (Kakoune-compatible).
pub static NORMAL_KEYMAP: &[KeyMapping] = &[
    // === Movement (basic) ===
    KeyMapping::new(key!('h'), Command::MoveLeft, "move left"),
    KeyMapping::new(key!('j'), Command::MoveDown, "move down"),
    KeyMapping::new(key!('k'), Command::MoveUp, "move up"),
    KeyMapping::new(key!('l'), Command::MoveRight, "move right"),
    KeyMapping::new(key!(special Left), Command::MoveLeft, "move left"),
    KeyMapping::new(key!(special Down), Command::MoveDown, "move down"),
    KeyMapping::new(key!(special Up), Command::MoveUp, "move up"),
    KeyMapping::new(key!(special Right), Command::MoveRight, "move right"),
    // === Movement (word) ===
    KeyMapping::new(key!('w'), Command::MoveNextWordStart, "select to next word start"),
    KeyMapping::new(key!('b'), Command::MovePrevWordStart, "select to previous word start"),
    KeyMapping::new(key!('e'), Command::MoveNextWordEnd, "select to next word end"),
    KeyMapping::new(key!(alt 'w'), Command::MoveNextWORDStart, "select to next WORD start"),
    KeyMapping::new(key!(alt 'b'), Command::MovePrevWORDStart, "select to previous WORD start"),
    KeyMapping::new(key!(alt 'e'), Command::MoveNextWORDEnd, "select to next WORD end"),
    // === Movement (line) ===
    KeyMapping::new(key!(alt 'h'), Command::MoveLineStart, "select to line begin"),
    KeyMapping::new(key!(alt 'l'), Command::MoveLineEnd, "select to line end"),
    KeyMapping::new(key!(special Home), Command::MoveLineStart, "select to line begin"),
    KeyMapping::new(key!(special End), Command::MoveLineEnd, "select to line end"),
    // === Find character ===
    KeyMapping::new(key!('f'), Command::FindCharForward { inclusive: true, ch: None }, "select to next char (inclusive)"),
    KeyMapping::new(key!('t'), Command::FindCharForward { inclusive: false, ch: None }, "select to next char (exclusive)"),
    KeyMapping::new(key!(alt 'f'), Command::FindCharBackward { inclusive: true, ch: None }, "select to prev char (inclusive)"),
    KeyMapping::new(key!(alt 't'), Command::FindCharBackward { inclusive: false, ch: None }, "select to prev char (exclusive)"),
    KeyMapping::new(key!(alt '.'), Command::RepeatLastFind, "repeat last object/find"),
    // === Selection manipulation ===
    KeyMapping::new(key!(';'), Command::CollapseSelection, "collapse selection to cursor"),
    KeyMapping::new(key!(alt ';'), Command::FlipSelection, "flip selection direction"),
    KeyMapping::new(key!(alt ':'), Command::EnsureForward, "ensure selection is forward"),
    KeyMapping::new(key!(','), Command::KeepPrimarySelection, "keep only primary selection"),
    KeyMapping::new(key!(alt ','), Command::RemovePrimarySelection, "remove primary selection"),
    KeyMapping::new(key!(')'), Command::RotateSelectionsForward, "rotate selections forward"),
    KeyMapping::new(key!('('), Command::RotateSelectionsBackward, "rotate selections backward"),
    // === Expand selections ===
    KeyMapping::new(key!('x'), Command::SelectLine, "select whole lines"),
    KeyMapping::new(key!(alt 'x'), Command::TrimToLine, "trim to whole lines"),
    KeyMapping::new(key!('%'), Command::SelectAll, "select whole buffer"),
    // === Changes ===
    KeyMapping::new(key!('d'), Command::Delete { yank: true }, "delete (yank)"),
    KeyMapping::new(key!(alt 'd'), Command::Delete { yank: false }, "delete (no yank)"),
    KeyMapping::new(key!('c'), Command::Change { yank: true }, "change (yank)"),
    KeyMapping::new(key!(alt 'c'), Command::Change { yank: false }, "change (no yank)"),
    KeyMapping::new(key!('y'), Command::Yank, "yank"),
    KeyMapping::new(key!('p'), Command::Paste { before: false }, "paste after"),
    KeyMapping::new(key!('P'), Command::Paste { before: true }, "paste before"),
    KeyMapping::new(key!(alt 'p'), Command::PasteAll { before: false }, "paste all after"),
    KeyMapping::new(key!(alt 'P'), Command::PasteAll { before: true }, "paste all before"),
    KeyMapping::new(key!('R'), Command::ReplaceWithYanked, "replace with yanked"),
    KeyMapping::new(key!('r'), Command::ReplaceWithChar, "replace with char"),
    // === Insert mode ===
    KeyMapping::new(key!('i'), Command::InsertBefore, "insert before"),
    KeyMapping::new(key!('a'), Command::InsertAfter, "insert after"),
    KeyMapping::new(key!('I'), Command::InsertLineStart, "insert at line start"),
    KeyMapping::new(key!('A'), Command::InsertLineEnd, "insert at line end"),
    KeyMapping::new(key!('o'), Command::OpenBelow, "open line below"),
    KeyMapping::new(key!('O'), Command::OpenAbove, "open line above"),
    KeyMapping::new(key!(alt 'o'), Command::AddEmptyLineBelow, "add empty line below"),
    KeyMapping::new(key!(alt 'O'), Command::AddEmptyLineAbove, "add empty line above"),
    // === Undo/Redo ===
    KeyMapping::new(key!('u'), Command::Undo, "undo"),
    KeyMapping::new(key!('U'), Command::Redo, "redo"),
    KeyMapping::new(key!(alt 'u'), Command::UndoSelectionChange, "undo selection change"),
    KeyMapping::new(key!(alt 'U'), Command::RedoSelectionChange, "redo selection change"),
    // === Search ===
    KeyMapping::new(key!('/'), Command::SearchForward, "search forward"),
    KeyMapping::new(key!('?'), Command::SearchBackward, "search extend forward"),
    KeyMapping::new(key!(alt '/'), Command::SearchBackward, "search backward"),
    KeyMapping::new(key!('n'), Command::SearchNext, "next match"),
    KeyMapping::new(key!('N'), Command::SearchNextAdd, "add next match"),
    KeyMapping::new(key!(alt 'n'), Command::SearchPrev, "previous match"),
    KeyMapping::new(key!(alt 'N'), Command::SearchPrevAdd, "add previous match"),
    KeyMapping::new(key!('*'), Command::UseSelectionAsSearch, "use selection as search pattern"),
    // === Macros ===
    KeyMapping::new(key!('Q'), Command::RecordMacro, "record/stop macro"),
    KeyMapping::new(key!('q'), Command::PlayMacro, "play macro"),
    // === Marks ===
    KeyMapping::new(key!('Z'), Command::SaveSelections, "save selections"),
    KeyMapping::new(key!('z'), Command::RestoreSelections, "restore selections"),
    // === Indent ===
    KeyMapping::new(key!('>'), Command::Indent, "indent"),
    KeyMapping::new(key!('<'), Command::Deindent, "deindent"),
    // === Case ===
    KeyMapping::new(key!('`'), Command::ToLowerCase, "to lowercase"),
    KeyMapping::new(key!('~'), Command::ToUpperCase, "to uppercase"),
    KeyMapping::new(key!(alt '`'), Command::SwapCase, "swap case"),
    // === Join ===
    KeyMapping::new(key!(alt 'j'), Command::JoinLines, "join lines"),
    KeyMapping::new(key!(alt 'J'), Command::JoinLinesSelect, "join lines (select spaces)"),
    // === Object selection ===
    KeyMapping::new(key!(alt 'i'), Command::SelectInnerObject { object_type: None }, "select inner object"),
    KeyMapping::new(key!(alt 'a'), Command::SelectAroundObject { object_type: None }, "select around object"),
    KeyMapping::new(key!('['), Command::SelectToObjectStart { object_type: None }, "select to object start"),
    KeyMapping::new(key!(']'), Command::SelectToObjectEnd { object_type: None }, "select to object end"),
    KeyMapping::new(key!('{'), Command::ExtendToObjectStart { object_type: None }, "extend to object start"),
    KeyMapping::new(key!('}'), Command::ExtendToObjectEnd { object_type: None }, "extend to object end"),
    KeyMapping::new(key!(alt '['), Command::SelectToObjectStart { object_type: None }, "select to inner object start"),
    KeyMapping::new(key!(alt ']'), Command::SelectToObjectEnd { object_type: None }, "select to inner object end"),
    // === Scrolling ===
    KeyMapping::new(key!(ctrl 'u'), Command::ScrollHalfPageUp, "scroll half page up"),
    KeyMapping::new(key!(ctrl 'd'), Command::ScrollHalfPageDown, "scroll half page down"),
    KeyMapping::new(key!(ctrl 'b'), Command::ScrollPageUp, "scroll page up"),
    KeyMapping::new(key!(ctrl 'f'), Command::ScrollPageDown, "scroll page down"),
    KeyMapping::new(key!(special PageUp), Command::ScrollPageUp, "scroll page up"),
    KeyMapping::new(key!(special PageDown), Command::ScrollPageDown, "scroll page down"),
    // === Jump list ===
    KeyMapping::new(key!(ctrl 'i'), Command::JumpForward, "jump forward"),
    KeyMapping::new(key!(ctrl 'o'), Command::JumpBackward, "jump backward"),
    KeyMapping::new(key!(ctrl 's'), Command::PushJump, "save jump"),
    // === Regex selection ===
    KeyMapping::new(key!('s'), Command::SelectRegex, "select regex matches"),
    KeyMapping::new(key!('S'), Command::SplitRegex, "split on regex"),
    KeyMapping::new(key!(alt 's'), Command::SplitLines, "split on lines"),
    KeyMapping::new(key!(alt 'k'), Command::KeepMatching, "keep matching"),
    KeyMapping::new(key!(alt 'K'), Command::KeepNotMatching, "keep not matching"),
    // === View/Goto ===
    KeyMapping::new(key!('v'), Command::EnterViewMode, "view mode"),
    KeyMapping::new(key!('V'), Command::EnterViewMode, "view mode (lock)"),
    KeyMapping::new(key!('g'), Command::EnterGotoMode, "goto mode"),
    KeyMapping::new(key!('G'), Command::EnterGotoMode, "extend goto mode"),
    // === Command ===
    KeyMapping::new(key!(':'), Command::EnterCommandMode, "command mode"),
    // === Pipe/Shell ===
    KeyMapping::new(key!('|'), Command::PipeReplace, "pipe (replace)"),
    KeyMapping::new(key!(alt '|'), Command::PipeIgnore, "pipe (ignore)"),
    KeyMapping::new(key!('!'), Command::InsertOutput, "insert command output"),
    KeyMapping::new(key!(alt '!'), Command::AppendOutput, "append command output"),
    // === Repeat ===
    KeyMapping::new(key!('.'), Command::RepeatLastInsert, "repeat last insert"),
    // === Misc ===
    KeyMapping::new(key!(ctrl 'l'), Command::ForceRedraw, "force redraw"),
    KeyMapping::new(key!('+'), Command::DuplicateSelections, "duplicate selections"),
    KeyMapping::new(key!(alt '+'), Command::MergeSelections, "merge overlapping selections"),
    KeyMapping::new(key!('&'), Command::Align, "align"),
    KeyMapping::new(key!(alt '&'), Command::CopyIndent, "copy indent"),
    KeyMapping::new(key!('@'), Command::TabsToSpaces, "tabs to spaces"),
    KeyMapping::new(key!(alt '@'), Command::SpacesToTabs, "spaces to tabs"),
    KeyMapping::new(key!('_'), Command::TrimSelections, "trim selections"),
    KeyMapping::new(key!('C'), Command::DuplicateSelectionsOnNextLines, "copy selections down"),
    KeyMapping::new(key!(alt 'C'), Command::DuplicateSelectionsOnPrevLines, "copy selections up"),
    // === User ===
    KeyMapping::new(key!(' '), Command::UserMappings, "user mappings"),
    // === Escape ===
    KeyMapping::new(key!(special Escape), Command::Escape, "escape"),
    // === Quit ===
    KeyMapping::new(key!(ctrl 'q'), Command::Quit, "quit"),
];

/// Goto mode keymap.
pub static GOTO_KEYMAP: &[KeyMapping] = &[
    KeyMapping::new(key!('h'), Command::MoveLineStart, "line begin"),
    KeyMapping::new(key!('l'), Command::MoveLineEnd, "line end"),
    KeyMapping::new(key!('i'), Command::MoveFirstNonWhitespace, "first non-blank"),
    KeyMapping::new(key!('g'), Command::MoveDocumentStart, "buffer top"),
    KeyMapping::new(key!('k'), Command::MoveDocumentStart, "buffer top"),
    KeyMapping::new(key!('j'), Command::MoveDocumentEnd, "buffer bottom"),
    KeyMapping::new(key!('e'), Command::MoveDocumentEnd, "buffer end"),
    // More goto commands can be added (gt, gc, gb, ga, gf, g.)
    KeyMapping::new(key!(special Escape), Command::Escape, "cancel"),
];

/// View mode keymap.
pub static VIEW_KEYMAP: &[KeyMapping] = &[
    KeyMapping::new(key!('v'), Command::ScrollUp, "center vertically"),
    KeyMapping::new(key!('c'), Command::ScrollUp, "center vertically"),
    KeyMapping::new(key!('t'), Command::ScrollUp, "cursor to top"),
    KeyMapping::new(key!('b'), Command::ScrollDown, "cursor to bottom"),
    KeyMapping::new(key!('h'), Command::ScrollUp, "scroll left"),
    KeyMapping::new(key!('j'), Command::ScrollDown, "scroll down"),
    KeyMapping::new(key!('k'), Command::ScrollUp, "scroll up"),
    KeyMapping::new(key!('l'), Command::ScrollDown, "scroll right"),
    KeyMapping::new(key!(special Escape), Command::Escape, "cancel"),
];

/// Object types for object selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Word,
    WORD,
    Sentence,
    Paragraph,
    Parentheses,
    Braces,
    Brackets,
    AngleBrackets,
    DoubleQuotes,
    SingleQuotes,
    Backticks,
    IndentBlock,
    Number,
    Argument,
    Whitespace,
    /// Custom delimiter character.
    Custom(char),
}

impl ObjectType {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'w' => Some(Self::Word),
            'b' | '(' | ')' => Some(Self::Parentheses),
            'B' | '{' | '}' => Some(Self::Braces),
            'r' | '[' | ']' => Some(Self::Brackets),
            'a' | '<' | '>' => Some(Self::AngleBrackets),
            'Q' | '"' => Some(Self::DoubleQuotes),
            'q' | '\'' => Some(Self::SingleQuotes),
            'g' | '`' => Some(Self::Backticks),
            's' => Some(Self::Sentence),
            'p' => Some(Self::Paragraph),
            'i' => Some(Self::IndentBlock),
            'n' => Some(Self::Number),
            'u' => Some(Self::Argument),
            ' ' => Some(Self::Whitespace),
            c if c.is_ascii_punctuation() => Some(Self::Custom(c)),
            _ => None,
        }
    }

    pub fn delimiters(&self) -> Option<(char, char)> {
        match self {
            Self::Parentheses => Some(('(', ')')),
            Self::Braces => Some(('{', '}')),
            Self::Brackets => Some(('[', ']')),
            Self::AngleBrackets => Some(('<', '>')),
            Self::DoubleQuotes => Some(('"', '"')),
            Self::SingleQuotes => Some(('\'', '\'')),
            Self::Backticks => Some(('`', '`')),
            Self::Custom(c) => Some((*c, *c)),
            _ => None,
        }
    }
}

/// Look up a key in a keymap.
pub fn lookup(keymap: &[KeyMapping], key: Key) -> Option<&KeyMapping> {
    keymap.iter().find(|m| m.key == key)
}

/// Look up a key in the normal mode keymap.
pub fn lookup_normal(key: Key) -> Option<&'static KeyMapping> {
    lookup(NORMAL_KEYMAP, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_basic_movement() {
        let h = lookup_normal(Key::char('h')).unwrap();
        assert_eq!(h.command, Command::MoveLeft);

        let j = lookup_normal(Key::char('j')).unwrap();
        assert_eq!(j.command, Command::MoveDown);
    }

    #[test]
    fn test_lookup_alt_key() {
        let alt_w = lookup_normal(Key::alt('w')).unwrap();
        assert_eq!(alt_w.command, Command::MoveNextWORDStart);
    }

    #[test]
    fn test_lookup_ctrl_key() {
        let ctrl_u = lookup_normal(Key::ctrl('u')).unwrap();
        assert_eq!(ctrl_u.command, Command::ScrollHalfPageUp);
    }

    #[test]
    fn test_lookup_unmapped() {
        assert!(lookup_normal(Key::char('Z')).is_some()); // Z is mapped
        assert!(lookup_normal(Key::ctrl('z')).is_none()); // Ctrl-z is not
    }

    #[test]
    fn test_object_type_from_char() {
        assert_eq!(ObjectType::from_char('w'), Some(ObjectType::Word));
        assert_eq!(ObjectType::from_char('('), Some(ObjectType::Parentheses));
        assert_eq!(ObjectType::from_char('"'), Some(ObjectType::DoubleQuotes));
    }

    #[test]
    fn test_keymap_coverage() {
        // Verify we have a substantial keymap
        assert!(NORMAL_KEYMAP.len() > 80);
        assert!(GOTO_KEYMAP.len() > 5);
    }
}
