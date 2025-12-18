//! Extension infrastructure using compile-time distributed slices.
//!
//! This module provides zero-cost registration using `linkme`.
//! Extensions are collected at link-time into static slices, requiring no
//! runtime initialization.
//!
//! # Extension Types
//!
//! - [`CommandDef`]: Named commands that can be executed (`:write`, `:quit`)
//! - [`MotionDef`]: Movement operations that modify selections
//! - [`TextObjectDef`]: Text object selectors (word, paragraph, quotes)
//! - [`FileTypeDef`]: File type detection and configuration
//! - [`HookDef`]: Event hooks for editor lifecycle events
//!
//! # Registration
//!
//! Use `#[distributed_slice(SLICE_NAME)]` to register extensions:
//!
//! ```ignore
//! use tome_core::ext::{CommandDef, COMMANDS};
//! use linkme::distributed_slice;
//!
//! #[distributed_slice(COMMANDS)]
//! static CMD_SAVE: CommandDef = CommandDef {
//!     name: "write",
//!     aliases: &["w"],
//!     description: "Save buffer to file",
//!     handler: |ctx| { /* ... */ Ok(()) },
//! };
//! ```

#[cfg(feature = "host")]
mod actions;
#[cfg(feature = "host")]
mod commands;
#[cfg(feature = "host")]
pub mod editor_ctx;
#[cfg(feature = "host")]
mod filetypes;
#[cfg(feature = "host")]
mod hooks;
#[cfg(feature = "host")]
mod keybindings;
#[cfg(feature = "host")]
pub mod macros;
#[cfg(feature = "host")]
mod motions;
#[cfg(feature = "host")]
mod objects;
#[cfg(feature = "host")]
mod options;
#[cfg(feature = "host")]
pub mod statusline;

#[cfg(feature = "host")]
pub use actions::{
	ACTIONS, ActionArgs, ActionContext, ActionDef, ActionHandler, ActionMode, ActionResult,
	EditAction, ObjectSelectionKind, PendingAction, PendingKind, ScrollAmount, ScrollDir,
	VisualDirection, execute_action, find_action,
};
#[cfg(feature = "host")]
pub use editor_ctx::{
	CursorAccess, EditAccess, EditorCapabilities, EditorContext, HandleOutcome, JumpAccess,
	MacroAccess, MessageAccess, ModeAccess, ResultHandler, ScratchAccess, SearchAccess,
	SelectionAccess, SelectionOpsAccess, TextAccess, TransformAccess, UndoAccess, dispatch_result,
};
#[cfg(feature = "host")]
pub use hooks::{
	HOOKS, HookContext, HookDef, HookEvent, HookResult, MUTABLE_HOOKS, MutableHookContext,
	MutableHookDef, all_hooks, emit as emit_hook, emit_mutable as emit_mutable_hook, find_hooks,
};
#[cfg(feature = "host")]
pub use keybindings::{
	BindingMode, KeyBindingDef, bindings_for_action, bindings_for_mode, find_binding,
};
#[cfg(feature = "host")]
use linkme::distributed_slice;
#[cfg(feature = "host")]
pub use options::{
	OPTIONS, OptionDef, OptionScope, OptionType, OptionValue, all_options, find_option,
};
#[cfg(feature = "host")]
use ropey::RopeSlice;
#[cfg(feature = "host")]
pub use statusline::{
	RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineContext,
	StatuslineSegmentDef, all_segments, find_segment, render_position, segments_for_position,
};

#[cfg(feature = "host")]
use crate::range::Range;
#[cfg(feature = "host")]
use crate::selection::Selection;

/// Result type for command execution.
#[cfg(feature = "host")]
pub type CommandResult = Result<(), CommandError>;

/// Error returned by command handlers.
#[cfg(feature = "host")]
#[derive(Debug, Clone)]
pub enum CommandError {
	/// Command failed with a message.
	Failed(String),
	/// Command requires an argument.
	MissingArgument(&'static str),
	/// Invalid argument provided.
	InvalidArgument(String),
	/// File I/O error.
	Io(String),
	/// Command not found.
	NotFound(String),
}

#[cfg(feature = "host")]
impl std::fmt::Display for CommandError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			CommandError::Failed(msg) => write!(f, "{}", msg),
			CommandError::MissingArgument(name) => write!(f, "missing argument: {}", name),
			CommandError::InvalidArgument(msg) => write!(f, "invalid argument: {}", msg),
			CommandError::Io(msg) => write!(f, "I/O error: {}", msg),
			CommandError::NotFound(name) => write!(f, "command not found: {}", name),
		}
	}
}

#[cfg(feature = "host")]
impl std::error::Error for CommandError {}

/// Result type for commands that may signal special behavior.
#[cfg(feature = "host")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
	/// Command completed normally.
	Ok,
	/// Command requests editor to quit.
	Quit,
	/// Command requests editor to quit without saving.
	ForceQuit,
}

/// Operations that commands can perform on the editor.
///
/// This trait abstracts editor functionality so commands can be defined
/// in `tome-core` without depending on the terminal layer.
#[cfg(feature = "host")]
pub trait EditorOps {
	/// Get the file path being edited, if any.
	fn path(&self) -> Option<&std::path::Path>;

	/// Get the document text as a rope slice.
	fn text(&self) -> RopeSlice<'_>;

	/// Get mutable access to selection.
	fn selection_mut(&mut self) -> &mut Selection;

	/// Display a message to the user.
	fn message(&mut self, msg: &str);

	/// Display an error message.
	fn error(&mut self, msg: &str);

	/// Save the buffer to disk.
	fn save(&mut self) -> Result<(), CommandError>;

	/// Save the buffer to a new file path.
	fn save_as(&mut self, path: std::path::PathBuf) -> Result<(), CommandError>;

	/// Insert text at the current selection.
	fn insert_text(&mut self, text: &str);

	/// Delete the current selection.
	fn delete_selection(&mut self);

	/// Mark that the buffer has been modified.
	fn set_modified(&mut self, modified: bool);

	/// Check if buffer is modified.
	fn is_modified(&self) -> bool;

	/// Set the editor theme.
	fn set_theme(&mut self, _theme_name: &str) -> Result<(), String> {
		Err("Theme switching not supported".to_string())
	}

	/// Handle a permission decision from the user.
	fn on_permission_decision(&mut self, _request_id: u64, _option_id: &str) -> Result<(), String> {
		Err("Permission handling not supported".to_string())
	}
}

/// Context passed to command handlers.
///
/// This provides access to editor state through the `EditorOps` trait,
/// allowing commands to perform real operations without depending on
/// the terminal layer.
#[cfg(feature = "host")]
pub struct CommandContext<'a> {
	/// Editor operations.
	pub editor: &'a mut dyn EditorOps,
	/// Command arguments (for `:command arg1 arg2`).
	pub args: &'a [&'a str],
	/// Numeric count prefix (1 if not specified).
	pub count: usize,
	/// Register to use (if any).
	pub register: Option<char>,
}

#[cfg(feature = "host")]
impl<'a> CommandContext<'a> {
	/// Convenience: get document text.
	pub fn text(&self) -> RopeSlice<'_> {
		self.editor.text()
	}

	/// Convenience: show a message.
	pub fn message(&mut self, msg: &str) {
		self.editor.message(msg);
	}

	/// Convenience: show an error.
	pub fn error(&mut self, msg: &str) {
		self.editor.error(msg);
	}
}

/// A named command that can be executed via command mode (`:name`).
///
/// Commands are the primary way to add functionality to Tome.
/// They can be invoked from the command line or bound to keys.
#[cfg(feature = "host")]
#[derive(Clone, Copy)]
pub struct CommandDef {
	/// Primary command name (e.g., "write").
	pub name: &'static str,
	/// Alternative names (e.g., &["w"] for write).
	pub aliases: &'static [&'static str],
	/// Short description for help.
	pub description: &'static str,
	/// Command handler function.
	pub handler: fn(&mut CommandContext) -> Result<CommandOutcome, CommandError>,
}

#[cfg(feature = "host")]
impl std::fmt::Debug for CommandDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CommandDef")
			.field("name", &self.name)
			.field("aliases", &self.aliases)
			.field("description", &self.description)
			.finish()
	}
}

/// Registry of all command definitions.
#[cfg(feature = "host")]
#[distributed_slice]
pub static COMMANDS: [CommandDef];

/// A motion that modifies the selection.
///
/// Motions are the building blocks of movement in Tome. Each motion
/// takes the current document and selection, and returns a new selection.
#[cfg(feature = "host")]
#[derive(Clone, Copy)]
pub struct MotionDef {
	/// Motion name for documentation/debugging.
	pub name: &'static str,
	/// Short description.
	pub description: &'static str,
	/// The motion function.
	///
	/// Parameters:
	/// - `text`: Document slice
	/// - `range`: Current range to move from
	/// - `count`: Repeat count (1 if not specified)
	/// - `extend`: If true, extend selection instead of moving
	///
	/// Returns the new range after applying the motion.
	pub handler: fn(text: RopeSlice, range: Range, count: usize, extend: bool) -> Range,
}

#[cfg(feature = "host")]
impl std::fmt::Debug for MotionDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MotionDef")
			.field("name", &self.name)
			.field("description", &self.description)
			.finish()
	}
}

/// Registry of all motion definitions.
#[cfg(feature = "host")]
#[distributed_slice]
pub static MOTIONS: [MotionDef];

/// A text object that can be selected.
///
/// Text objects define regions of text (word, sentence, quoted string, etc.)
/// that can be selected with `inner` or `around` variants.
#[cfg(feature = "host")]
#[derive(Clone, Copy)]
pub struct TextObjectDef {
	/// Object name for documentation.
	pub name: &'static str,
	/// Character that triggers this object (e.g., 'w' for word).
	pub trigger: char,
	/// Alternative trigger characters (e.g., '(' and ')' both select parentheses).
	pub alt_triggers: &'static [char],
	/// Short description.
	pub description: &'static str,
	/// Select the inner content (without delimiters).
	///
	/// Parameters:
	/// - `text`: Document slice
	/// - `pos`: Cursor position
	///
	/// Returns the range of the inner content, or None if not applicable.
	pub inner: fn(text: RopeSlice, pos: usize) -> Option<Range>,
	/// Select around the object (including delimiters).
	pub around: fn(text: RopeSlice, pos: usize) -> Option<Range>,
}

#[cfg(feature = "host")]
impl std::fmt::Debug for TextObjectDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("TextObjectDef")
			.field("name", &self.name)
			.field("trigger", &self.trigger)
			.field("alt_triggers", &self.alt_triggers)
			.field("description", &self.description)
			.finish()
	}
}

/// Registry of all text object definitions.
#[cfg(feature = "host")]
#[distributed_slice]
pub static TEXT_OBJECTS: [TextObjectDef];

/// File type definition for language-specific configuration.
#[cfg(feature = "host")]
#[derive(Clone, Copy)]
pub struct FileTypeDef {
	/// File type name (e.g., "rust", "python").
	pub name: &'static str,
	/// File extensions that match this type.
	pub extensions: &'static [&'static str],
	/// File name patterns (e.g., "Makefile", ".gitignore").
	pub filenames: &'static [&'static str],
	/// First-line patterns for shebang detection.
	pub first_line_patterns: &'static [&'static str],
	/// Short description.
	pub description: &'static str,
}

#[cfg(feature = "host")]
impl std::fmt::Debug for FileTypeDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FileTypeDef")
			.field("name", &self.name)
			.field("extensions", &self.extensions)
			.finish()
	}
}

/// Registry of all file type definitions.
#[cfg(feature = "host")]
#[distributed_slice]
pub static FILE_TYPES: [FileTypeDef];

/// Look up a command by name or alias.
#[cfg(feature = "host")]
pub fn find_command(name: &str) -> Option<&'static CommandDef> {
	COMMANDS
		.iter()
		.find(|cmd| cmd.name == name || cmd.aliases.contains(&name))
}

/// Look up a motion by name.
#[cfg(feature = "host")]
pub fn find_motion(name: &str) -> Option<&'static MotionDef> {
	MOTIONS.iter().find(|m| m.name == name)
}

/// Look up a text object by trigger character.
#[cfg(feature = "host")]
pub fn find_text_object(trigger: char) -> Option<&'static TextObjectDef> {
	TEXT_OBJECTS
		.iter()
		.find(|obj| obj.trigger == trigger || obj.alt_triggers.contains(&trigger))
}

/// Detect file type from filename.
///
/// Uses a two-step detection process:
/// 1. First checks for exact filename matches (e.g., "Makefile", "Cargo.toml")
/// 2. Falls back to extension-based detection (e.g., ".rs", ".py")
///
/// # Examples
/// ```ignore
/// // Exact filename match
/// detect_file_type("Makefile") // Returns makefile type
///
/// // Extension-based match
/// detect_file_type("/path/to/main.rs") // Returns rust type
/// detect_file_type("script.py") // Returns python type
/// ```
#[cfg(feature = "host")]
pub fn detect_file_type(filename: &str) -> Option<&'static FileTypeDef> {
	let basename = filename.rsplit('/').next().unwrap_or(filename);

	if let Some(ft) = FILE_TYPES
		.iter()
		.find(|ft| ft.filenames.contains(&basename))
	{
		return Some(ft);
	}

	if let Some(ext) = basename.rsplit('.').next()
		&& let Some(ft) = FILE_TYPES.iter().find(|ft| ft.extensions.contains(&ext))
	{
		return Some(ft);
	}

	None
}

/// Detect file type from first line (shebang).
#[cfg(feature = "host")]
pub fn detect_file_type_from_content(first_line: &str) -> Option<&'static FileTypeDef> {
	FILE_TYPES.iter().find(|ft| {
		ft.first_line_patterns
			.iter()
			.any(|pattern| first_line.contains(pattern))
	})
}

#[cfg(all(test, feature = "host"))]
mod tests {
	use super::*;

	#[test]
	fn test_distributed_slices_accessible() {
		// Verify builtin registrations are present
		assert!(TEXT_OBJECTS.len() >= 13); // word, WORD, parens, braces, brackets, angle, quotes x3, line, paragraph, argument, number
		assert!(FILE_TYPES.len() >= 25); // rust, python, js, ts, c, cpp, go, java, data formats, web, docs, shell, config
		assert!(MOTIONS.len() >= 10); // basic movement, word, line, document
		assert!(COMMANDS.len() >= 5); // quit, write, edit, buffer commands
		assert!(OPTIONS.len() >= 15); // indent, display, scroll, search, file, behavior options
	}

	#[test]
	fn test_find_text_object() {
		let word = find_text_object('w').expect("word object should exist");
		assert_eq!(word.name, "word");

		let parens = find_text_object('(').expect("parens object should exist via alt trigger");
		assert_eq!(parens.name, "parentheses");

		let parens2 =
			find_text_object('b').expect("parens object should exist via primary trigger");
		assert_eq!(parens2.name, "parentheses");
	}

	#[test]
	fn test_new_text_objects() {
		let line = find_text_object('x').expect("line object should exist");
		assert_eq!(line.name, "line");

		let para = find_text_object('p').expect("paragraph object should exist");
		assert_eq!(para.name, "paragraph");

		let arg = find_text_object('c').expect("argument object should exist");
		assert_eq!(arg.name, "argument");

		let num = find_text_object('n').expect("number object should exist");
		assert_eq!(num.name, "number");
	}

	#[test]
	fn test_detect_file_type() {
		let rust = detect_file_type("main.rs").expect("should detect rust");
		assert_eq!(rust.name, "rust");

		let python = detect_file_type("/path/to/script.py").expect("should detect python");
		assert_eq!(python.name, "python");

		let makefile = detect_file_type("Makefile").expect("should detect makefile");
		assert_eq!(makefile.name, "makefile");
	}

	#[test]
	fn test_command_error_display() {
		let err = CommandError::Failed("test error".into());
		assert_eq!(format!("{}", err), "test error");

		let err = CommandError::MissingArgument("filename");
		assert_eq!(format!("{}", err), "missing argument: filename");
	}

	#[test]
	fn test_find_command() {
		let quit = find_command("quit").expect("quit command should exist");
		assert_eq!(quit.name, "quit");

		let quit_alias = find_command("q").expect("q alias should find quit");
		assert_eq!(quit_alias.name, "quit");

		let write = find_command("w").expect("w alias should find write");
		assert_eq!(write.name, "write");
	}

	#[test]
	fn test_find_motion() {
		let left = find_motion("move_left").expect("move_left motion should exist");
		assert_eq!(left.name, "move_left");

		let word = find_motion("next_word_start").expect("next_word_start motion should exist");
		assert_eq!(word.name, "next_word_start");
	}

	#[test]
	fn test_hooks_accessible() {
		assert!(HOOKS.len() >= 2, "should have at least 2 builtin hooks");

		let hooks = all_hooks();
		assert!(hooks.len() >= 2);

		// find_hooks should find our mode change hook
		let mode_hooks: Vec<_> = find_hooks(HookEvent::ModeChange).collect();
		assert!(!mode_hooks.is_empty(), "should have mode change hooks");

		// find_hooks should find our buffer open hook
		let open_hooks: Vec<_> = find_hooks(HookEvent::BufferOpen).collect();
		assert!(!open_hooks.is_empty(), "should have buffer open hooks");
	}

	#[test]
	fn test_emit_hook() {
		use crate::Mode;

		// This should not panic even with no handlers
		let ctx = HookContext::ModeChange {
			old_mode: Mode::Normal,
			new_mode: Mode::Insert,
		};
		emit_hook(&ctx);
	}
}
