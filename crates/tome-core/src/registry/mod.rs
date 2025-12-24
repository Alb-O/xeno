//! Registry infrastructure using compile-time distributed slices.
//!
//! This module provides zero-cost registration using `linkme`.
//! Registry items are collected at link-time into static slices, requiring no
//! runtime initialization.

#[cfg(feature = "host")]
use linkme::distributed_slice;

/// Represents where a registry item was defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg(feature = "host")]
pub enum RegistrySource {
	/// Built directly into the tome-core crate.
	Builtin,
	/// Defined in a library crate.
	Crate(&'static str),
}

#[cfg(feature = "host")]
impl std::fmt::Display for RegistrySource {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Builtin => write!(f, "builtin"),
			Self::Crate(name) => write!(f, "crate:{}", name),
		}
	}
}

/// Represents an editor capability required by a registry item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg(feature = "host")]
pub enum Capability {
	/// Basic text access (reading content).
	Text,
	/// Cursor movement and querying.
	Cursor,
	/// Selection manipulation.
	Selection,
	/// Mode switching.
	Mode,
	/// Showing messages/notifications to the user.
	Messaging,
	/// Full edit access (modifying content).
	Edit,
	/// Search and find/replace.
	Search,
	/// Undo and redo history.
	Undo,
	/// Advanced selection operations (e.g. multi-cursor, select all).
	SelectionOps,
	/// Jumping to locations (e.g. definitions).
	Jump,
	/// Recording and playing macros.
	Macro,
	/// Applying transformations to text.
	Transform,
}

/// # Precedence Rules
///
/// When multiple registry items register with the same name, alias, or trigger,
/// the following rules determine the "winner" that will be indexed:
///
/// 1. **Priority**: Higher `priority` values always win.
/// 2. **ID Tie-break**: If priorities are equal, the one with the alphabetically
///    smaller `id` wins (e.g., `tome-core::quit` wins over `user-extension::quit`).
///
/// Collisions are recorded in the registry and can be inspected via `:registry diag`
/// or `:registry doctor`. In debug builds, collisions are logged as errors.
///
/// # Registry Types
///
/// | Registry | Key Type | Winner Selection | Collisions |
/// |----------|----------|------------------|------------|
/// | Commands | Name/Alias | Priority > ID | Warned |
/// | Actions  | Name | Priority > ID | Warned |
/// | Motions  | Name | Priority > ID | Warned |
/// | Text Objects | Trigger/Name | Priority > ID | Warned |
/// | Hooks | Event | All handlers run (sorted by priority) | N/A |
/// | File Types | Extension/Filename | Priority > ID | Warned |
///
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
pub mod index;
#[cfg(feature = "host")]
mod keybindings;
#[cfg(feature = "host")]
pub mod macros;
#[cfg(feature = "host")]
mod motions;
#[cfg(feature = "host")]
pub mod notifications;
#[cfg(feature = "host")]
mod objects;
#[cfg(feature = "host")]
mod options;
#[cfg(feature = "host")]
pub mod statusline;

#[cfg(feature = "host")]
pub use actions::{
	ACTIONS, ActionArgs, ActionContext, ActionDef, ActionHandler, ActionId, ActionMode,
	ActionResult, EditAction, ObjectSelectionKind, PendingAction, PendingKind, ScrollAmount,
	ScrollDir, VisualDirection, execute_action,
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
	BindingMode, KeyBindingDef, ResolvedBinding, bindings_for_action, bindings_for_mode,
	find_binding, find_binding_resolved,
};
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

/// Metadata common to all registry types.
#[cfg(feature = "host")]
pub trait RegistryMetadata {
	fn id(&self) -> &'static str;
	fn name(&self) -> &'static str;
	fn priority(&self) -> i16;
	fn source(&self) -> RegistrySource;
}

#[cfg(feature = "host")]
impl RegistryMetadata for CommandDef {
	fn id(&self) -> &'static str {
		self.id
	}
	fn name(&self) -> &'static str {
		self.name
	}
	fn priority(&self) -> i16 {
		self.priority
	}
	fn source(&self) -> RegistrySource {
		self.source
	}
}

#[cfg(feature = "host")]
impl RegistryMetadata for ActionDef {
	fn id(&self) -> &'static str {
		self.id
	}
	fn name(&self) -> &'static str {
		self.name
	}
	fn priority(&self) -> i16 {
		self.priority
	}
	fn source(&self) -> RegistrySource {
		self.source
	}
}

#[cfg(feature = "host")]
impl RegistryMetadata for MotionDef {
	fn id(&self) -> &'static str {
		self.id
	}
	fn name(&self) -> &'static str {
		self.name
	}
	fn priority(&self) -> i16 {
		self.priority
	}
	fn source(&self) -> RegistrySource {
		self.source
	}
}

#[cfg(feature = "host")]
impl RegistryMetadata for TextObjectDef {
	fn id(&self) -> &'static str {
		self.id
	}
	fn name(&self) -> &'static str {
		self.name
	}
	fn priority(&self) -> i16 {
		self.priority
	}
	fn source(&self) -> RegistrySource {
		self.source
	}
}

#[cfg(feature = "host")]
impl RegistryMetadata for FileTypeDef {
	fn id(&self) -> &'static str {
		self.id
	}
	fn name(&self) -> &'static str {
		self.name
	}
	fn priority(&self) -> i16 {
		self.priority
	}
	fn source(&self) -> RegistrySource {
		self.source
	}
}

#[cfg(feature = "host")]
impl std::fmt::Display for Capability {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Text => write!(f, "text"),
			Self::Cursor => write!(f, "cursor"),
			Self::Selection => write!(f, "selection"),
			Self::Mode => write!(f, "mode"),
			Self::Messaging => write!(f, "messaging"),
			Self::Edit => write!(f, "edit"),
			Self::Search => write!(f, "search"),
			Self::Undo => write!(f, "undo"),
			Self::SelectionOps => write!(f, "selection_ops"),
			Self::Jump => write!(f, "jump"),
			Self::Macro => write!(f, "macro"),
			Self::Transform => write!(f, "transform"),
		}
	}
}

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
	/// Missing capability required for this operation.
	#[error("missing capability: {0}")]
	MissingCapability(Capability),
	/// Operation not supported by this editor implementation.
	#[error("unsupported operation: {0}")]
	Unsupported(&'static str),
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
pub trait EditorOps: EditorCapabilities {
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
		Err(CommandError::Unsupported("set_theme"))
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

/// Flags for registry item behavior and metadata.
#[cfg(feature = "host")]
pub mod flags {
	pub const NONE: u32 = 0;
	/// Hidden from help and completion.
	pub const HIDDEN: u32 = 1 << 0;
	/// Mark as experimental.
	pub const EXPERIMENTAL: u32 = 1 << 1;
	/// Mark as potentially unsafe.
	pub const UNSAFE: u32 = 1 << 2;
}

/// A named command that can be executed via command mode (`:name`).
#[cfg(feature = "host")]
#[derive(Clone, Copy)]
pub struct CommandDef {
	/// Unique identifier (usually same as name).
	pub id: &'static str,
	/// Primary command name (e.g., "write").
	pub name: &'static str,
	/// Alternative names (e.g., &["w"] for write).
	pub aliases: &'static [&'static str],
	/// Short description for help.
	pub description: &'static str,
	/// Command handler function.
	pub handler: fn(_: &mut CommandContext<'_>) -> Result<CommandOutcome, CommandError>,
	/// Optional user data passed to the handler.
	pub user_data: Option<&'static (dyn std::any::Any + Sync)>,
	/// Priority for resolving name collisions.
	pub priority: i16,
	/// Origin of the command.
	pub source: RegistrySource,
	/// Capabilities required to run this command.
	pub required_caps: &'static [Capability],
	/// Metadata flags.
	pub flags: u32,
}

#[cfg(feature = "host")]
impl std::fmt::Debug for CommandDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CommandDef")
			.field("id", &self.id)
			.field("name", &self.name)
			.field("aliases", &self.aliases)
			.field("description", &self.description)
			.field("priority", &self.priority)
			.field("source", &self.source)
			.field("required_caps", &self.required_caps)
			.field("flags", &self.flags)
			.finish()
	}
}

/// Distributed slice of all registered commands.
#[cfg(feature = "host")]
#[distributed_slice]
pub static COMMANDS: [CommandDef];

/// A motion that modifies the selection.
#[cfg(feature = "host")]
#[derive(Clone, Copy, Debug)]
pub struct MotionDef {
	/// Unique identifier.
	pub id: &'static str,
	/// Name used to identify the motion.
	pub name: &'static str,
	/// Alternative names for the motion.
	pub aliases: &'static [&'static str],
	/// Short description for help.
	pub description: &'static str,
	/// Motion handler function.
	pub handler: fn(ropey::RopeSlice, crate::range::Range, usize, bool) -> crate::range::Range,
	/// Priority for resolving name collisions.
	pub priority: i16,
	/// Origin of the motion.
	pub source: RegistrySource,
	/// Capabilities required to run this motion.
	pub required_caps: &'static [Capability],
	/// Metadata flags.
	pub flags: u32,
}

/// Distributed slice of all registered motions.
#[cfg(feature = "host")]
#[distributed_slice]
pub static MOTIONS: [MotionDef];

/// A text object that can be selected.
#[cfg(feature = "host")]
#[derive(Clone, Copy, Debug)]
pub struct TextObjectDef {
	/// Unique identifier.
	pub id: &'static str,
	/// Name used to identify the text object.
	pub name: &'static str,
	/// Alternative names for the text object.
	pub aliases: &'static [&'static str],
	/// Character trigger (e.g., 'w' for word).
	pub trigger: char,
	/// Alternative character triggers.
	pub alt_triggers: &'static [char],
	/// Short description for help.
	pub description: &'static str,
	/// Function to select the inner part of the object.
	pub inner: fn(ropey::RopeSlice, usize) -> Option<crate::range::Range>,
	/// Function to select the object including surrounding whitespace/delimiters.
	pub around: fn(ropey::RopeSlice, usize) -> Option<crate::range::Range>,
	/// Priority for resolving trigger collisions.
	pub priority: i16,
	/// Origin of the text object.
	pub source: RegistrySource,
	/// Capabilities required to run this text object.
	pub required_caps: &'static [Capability],
	/// Metadata flags.
	pub flags: u32,
}

/// Distributed slice of all registered text objects.
#[cfg(feature = "host")]
#[distributed_slice]
pub static TEXT_OBJECTS: [TextObjectDef];

/// File type definition for language-specific configuration.
#[cfg(feature = "host")]
#[derive(Clone, Copy)]
pub struct FileTypeDef {
	/// Unique identifier.
	pub id: &'static str,
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
	/// Priority for resolving collisions.
	pub priority: i16,
	/// Origin of the file type.
	pub source: RegistrySource,
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
pub use index::{
	all_actions, all_commands, all_motions, all_text_objects, find_action, find_action_by_id,
	find_command, find_motion, find_text_object_by_name, find_text_object_by_trigger, get_registry,
	resolve_action_id,
};

/// Detect file type from filename.
#[cfg(feature = "host")]
pub fn detect_file_type(filename: &str) -> Option<&'static FileTypeDef> {
	let reg = get_registry();
	let basename = filename.rsplit('/').next().unwrap_or(filename);

	// Check filenames/extensions via by_alias index
	if let Some(ft) = reg.file_types.by_alias.get(basename) {
		return Some(ft);
	}

	if let Some(ext) = basename.rsplit('.').next()
		&& let Some(ft) = reg.file_types.by_alias.get(ext)
	{
		return Some(ft);
	}

	None
}

/// Detect file type from first line (shebang).
#[cfg(feature = "host")]
pub fn detect_file_type_from_content(first_line: &str) -> Option<&'static FileTypeDef> {
	let reg = get_registry();
	reg.file_types
		.by_name
		.values()
		.find(|ft| {
			ft.first_line_patterns
				.iter()
				.any(|pattern| first_line.contains(pattern))
		})
		.copied()
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
		let word = find_text_object_by_trigger('w').expect("word object should exist");
		assert_eq!(word.name, "word");

		let parens =
			find_text_object_by_trigger('(').expect("parens object should exist via alt trigger");
		assert_eq!(parens.name, "parentheses");

		let parens2 = find_text_object_by_trigger('b')
			.expect("parens object should exist via primary trigger");
		assert_eq!(parens2.name, "parentheses");
	}

	#[test]
	fn test_new_text_objects() {
		let line = find_text_object_by_trigger('x').expect("line object should exist");
		assert_eq!(line.name, "line");

		let para = find_text_object_by_trigger('p').expect("paragraph object should exist");
		assert_eq!(para.name, "paragraph");

		let arg = find_text_object_by_trigger('c').expect("argument object should exist");
		assert_eq!(arg.name, "argument");

		let num = find_text_object_by_trigger('n').expect("number object should exist");
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
