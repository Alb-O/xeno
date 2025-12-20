//! Extension infrastructure using compile-time distributed slices.
//!
//! This module provides zero-cost registration using `linkme`.
//! Extensions are collected at link-time into static slices, requiring no
//! runtime initialization.

#[cfg(feature = "host")]
mod actions;
#[cfg(feature = "host")]
mod commands;
#[cfg(feature = "host")]
mod completion;
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
pub use completion::{
	CommandSource, CompletionContext, CompletionItem, CompletionKind, CompletionSource,
};
#[cfg(feature = "host")]
pub use editor_ctx::{
	CursorAccess, EditAccess, EditorCapabilities, EditorContext, HandleOutcome, JumpAccess,
	MacroAccess, MessageAccess, ModeAccess, ResultHandler, SearchAccess, SelectionAccess,
	SelectionOpsAccess, TextAccess, TransformAccess, UndoAccess, dispatch_result,
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
use crate::range::{CharIdx, Range};

/// Result type for command execution.
#[cfg(feature = "host")]
pub type CommandResult = Result<(), CommandError>;

/// Error returned by command handlers.
#[cfg(feature = "host")]
#[derive(thiserror::Error, Debug, Clone)]
pub enum CommandError {
	/// Command failed with a message.
	#[error("{0}")]
	Failed(String),
	/// Command requires an argument.
	#[error("missing argument: {0}")]
	MissingArgument(&'static str),
	/// Invalid argument provided.
	#[error("invalid argument: {0}")]
	InvalidArgument(String),
	/// File I/O error.
	#[error("I/O error: {0}")]
	Io(String),
	/// Command not found.
	#[error("command not found: {0}")]
	NotFound(String),
	/// General error.
	#[error("{0}")]
	Other(String),
}

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

/// Operations that editors must support.
#[cfg(feature = "host")]
pub trait EditorOps:
	CursorAccess + SelectionAccess + TextAccess + ModeAccess + MessageAccess
{
	/// Get the file path being edited, if any.
	fn path(&self) -> Option<&std::path::Path>;

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
	fn set_theme(&mut self, _theme_name: &str) -> Result<(), CommandError> {
		Err(CommandError::Failed("Theme switching not supported".to_string()))
	}

	/// Handle a permission decision from the user.
	fn on_permission_decision(
		&mut self,
		_request_id: u64,
		_option_id: &str,
	) -> Result<(), CommandError> {
		Err(CommandError::Failed("Permission handling not supported".to_string()))
	}

	/// Execute a plugin-related command.
	fn plugin_command(&mut self, _args: &[&str]) -> Result<(), CommandError> {
		Err(CommandError::Failed("Plugin commands not supported".to_string()))
	}
}

/// Context passed to command handlers.
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
	/// User data from the command definition.
	pub user_data: Option<&'static (dyn std::any::Any + Sync)>,
}

#[cfg(feature = "host")]
impl<'a> CommandContext<'a> {
	/// Convenience: get document text.
	pub fn text(&self) -> RopeSlice<'_> {
		self.editor.text()
	}

	/// Convenience: show a message.
	pub fn message(&mut self, msg: &str) {
		self.editor.show_message(msg);
	}

	/// Convenience: show an error.
	pub fn error(&mut self, msg: &str) {
		self.editor.show_error(msg);
	}

	/// Try to retrieve typed user data from the command definition.
	pub fn require_user_data<T: std::any::Any + Sync>(&self) -> Result<&'static T, CommandError> {
		self.user_data
			.and_then(|d| {
				let any: &dyn std::any::Any = d;
				any.downcast_ref::<T>()
			})
			.ok_or_else(|| {
				CommandError::Other(format!(
					"Missing or invalid user data for command (expected {})",
					std::any::type_name::<T>()
				))
			})
	}
}

/// A named command that can be executed via command mode (`:name`).
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
	/// Optional user data passed to the handler.
	pub user_data: Option<&'static (dyn std::any::Any + Sync)>,
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
#[cfg(feature = "host")]
#[derive(Clone, Copy)]
pub struct MotionDef {
	/// Motion name for documentation/debugging.
	pub name: &'static str,
	/// Short description.
	pub description: &'static str,
	/// The motion function.
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
#[cfg(feature = "host")]
#[derive(Clone, Copy)]
pub struct TextObjectDef {
	/// Object name for documentation.
	pub name: &'static str,
	/// Character that triggers this object (e.g., 'w' for word).
	pub trigger: char,
	/// Alternative trigger characters.
	pub alt_triggers: &'static [char],
	/// Short description.
	pub description: &'static str,
	/// Select the inner content (without delimiters).
	pub inner: fn(text: RopeSlice, pos: CharIdx) -> Option<Range>,
	/// Select around the object (including delimiters).
	pub around: fn(text: RopeSlice, pos: CharIdx) -> Option<Range>,
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
		assert!(TEXT_OBJECTS.len() >= 13);
		assert!(FILE_TYPES.len() >= 25);
		assert!(MOTIONS.len() >= 10);
		assert!(COMMANDS.len() >= 5);
		assert!(OPTIONS.len() >= 15);
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
