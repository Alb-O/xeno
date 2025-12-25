//! Registry infrastructure using compile-time distributed slices.
//!
//! This crate contains type definitions, traits, and distributed slices for the
//! tome editor's extension system. It does NOT contain implementations - those
//! live in tome-stdlib.
//!
//! # Core Types
//!
//! - [`RegistrySource`] - Where a registry item was defined
//! - [`Capability`] - Editor capabilities required by registry items
//! - [`ActionId`] - Unique identifier for actions
//! - [`CommandError`] / [`CommandResult`] - Command execution errors
//!
//! # Modules
//!
//! - [`actions`] - Action definitions and handlers
//! - [`commands`] - Ex-mode command definitions
//! - [`hooks`] - Event lifecycle observers
//! - [`keybindings`] - Key to action mappings
//! - [`notifications`] - UI notification system
//!
//! # Distributed Slices
//!
//! Compile-time registries using [`linkme`]:
//! - [`ACTIONS`] - All registered actions
//! - [`COMMANDS`] - All registered commands
//! - [`MOTIONS`] - All registered motions
//! - [`TEXT_OBJECTS`] - All registered text objects
//! - [`FILE_TYPES`] - All registered file types

use linkme::distributed_slice;

pub use tome_base::range::CharIdx;
pub use tome_base::{Range, Selection};

/// Represents where a registry item was defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistrySource {
	/// Built directly into the editor.
	Builtin,
	/// Defined in a library crate.
	Crate(&'static str),
}

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
pub enum Capability {
	Text,
	Cursor,
	Selection,
	Mode,
	Messaging,
	Edit,
	Search,
	Undo,
	SelectionOps,
	Jump,
	Macro,
	Transform,
	FileOps,
}

impl std::fmt::Display for Capability {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Capability::Text => write!(f, "Text"),
			Capability::Cursor => write!(f, "Cursor"),
			Capability::Selection => write!(f, "Selection"),
			Capability::Mode => write!(f, "Mode"),
			Capability::Messaging => write!(f, "Messaging"),
			Capability::Edit => write!(f, "Edit"),
			Capability::Search => write!(f, "Search"),
			Capability::Undo => write!(f, "Undo"),
			Capability::SelectionOps => write!(f, "SelectionOps"),
			Capability::Jump => write!(f, "Jump"),
			Capability::Macro => write!(f, "Macro"),
			Capability::Transform => write!(f, "Transform"),
			Capability::FileOps => write!(f, "FileOps"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionId(pub u32);

impl ActionId {
	pub const INVALID: ActionId = ActionId(u32::MAX);
	#[inline]
	pub fn is_valid(self) -> bool {
		self != Self::INVALID
	}
	#[inline]
	pub fn as_u32(self) -> u32 {
		self.0
	}
}

impl std::fmt::Display for ActionId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if *self == Self::INVALID {
			write!(f, "ActionId(INVALID)")
		} else {
			write!(f, "ActionId({})", self.0)
		}
	}
}

/// Common semantic color identifiers.
pub const SEMANTIC_INFO: &str = "info";
pub const SEMANTIC_WARNING: &str = "warning";
pub const SEMANTIC_ERROR: &str = "error";
pub const SEMANTIC_SUCCESS: &str = "success";
pub const SEMANTIC_DIM: &str = "dim";
pub const SEMANTIC_NORMAL: &str = "normal";

pub trait RegistryMetadata {
	fn id(&self) -> &'static str;
	fn name(&self) -> &'static str;
	fn priority(&self) -> i16;
	fn source(&self) -> RegistrySource;
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum CommandError {
	#[error("{0}")]
	Failed(String),
	#[error("missing argument: {0}")]
	MissingArgument(&'static str),
	#[error("invalid argument: {0}")]
	InvalidArgument(String),
	#[error("I/O error: {0}")]
	Io(String),
	#[error("command not found: {0}")]
	NotFound(String),
	#[error("missing capability: {0:?}")]
	MissingCapability(Capability),
	#[error("unsupported operation: {0}")]
	Unsupported(&'static str),
	#[error("{0}")]
	Other(String),
}

pub type CommandResult = Result<(), CommandError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
	Ok,
	Quit,
	ForceQuit,
}

pub mod actions;
pub mod commands;
pub mod completion;
pub mod editor_ctx;
pub mod hooks;
pub mod index;
pub mod keybindings;
pub mod macros;
pub mod mode;
pub mod notifications;
pub mod options;
pub mod statusline;

#[distributed_slice]
pub static ACTIONS: [actions::ActionDef];

#[distributed_slice]
pub static COMMANDS: [commands::CommandDef];

/// Motion definition.
pub struct MotionDef {
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub description: &'static str,
	pub handler: fn(ropey::RopeSlice, Range, usize, bool) -> Range,
	pub priority: i16,
	pub source: RegistrySource,
	pub required_caps: &'static [Capability],
	pub flags: u32,
}

#[distributed_slice]
pub static MOTIONS: [MotionDef];

/// Text object definition.
pub struct TextObjectDef {
	pub id: &'static str,
	pub name: &'static str,
	pub aliases: &'static [&'static str],
	pub trigger: char,
	pub alt_triggers: &'static [char],
	pub description: &'static str,
	pub inner: fn(ropey::RopeSlice, usize) -> Option<Range>,
	pub around: fn(ropey::RopeSlice, usize) -> Option<Range>,
	pub priority: i16,
	pub source: RegistrySource,
	pub required_caps: &'static [Capability],
	pub flags: u32,
}

#[distributed_slice]
pub static TEXT_OBJECTS: [TextObjectDef];

/// File type definition.
pub struct FileTypeDef {
	pub id: &'static str,
	pub name: &'static str,
	pub extensions: &'static [&'static str],
	pub filenames: &'static [&'static str],
	pub first_line_patterns: &'static [&'static str],
	pub description: &'static str,
	pub priority: i16,
	pub source: RegistrySource,
}

#[distributed_slice]
pub static FILE_TYPES: [FileTypeDef];

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

pub use actions::{
	ActionArgs, ActionContext, ActionDef, ActionHandler, ActionMode, ActionResult, EditAction,
	ObjectSelectionKind, PendingAction, PendingKind, ScrollAmount, ScrollDir, VisualDirection,
};
pub use commands::{CommandContext, CommandDef, flags};
pub use completion::{CompletionContext, CompletionItem, CompletionKind, CompletionSource};
pub use editor_ctx::{
	EditorCapabilities, EditorContext, EditorOps, HandleOutcome, dispatch_result,
};
pub use hooks::{
	HOOKS, HookContext, HookDef, HookEvent, HookResult, MUTABLE_HOOKS, MutableHookContext,
	MutableHookDef, all_hooks, emit as emit_hook, emit_mutable as emit_mutable_hook, find_hooks,
};
pub use index::{
	all_actions, all_commands, all_motions, all_text_objects, find_action, find_action_by_id,
	find_command, find_motion, find_text_object_by_name, find_text_object_by_trigger,
	resolve_action_id,
};
pub use keybindings::{
	BindingMode, KeyBindingDef, ResolvedBinding, bindings_for_action, bindings_for_mode,
	find_binding, find_binding_resolved,
};
pub use mode::Mode;
pub use notifications::{
	Animation, AutoDismiss, Level, NOTIFICATION_TYPES, NotificationTypeDef, Timing,
	find_notification_type,
};
pub use options::{
	OPTIONS, OptionDef, OptionScope, OptionType, OptionValue, all_options, find_option,
};
pub use statusline::{
	RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineContext,
	StatuslineSegmentDef, all_segments, find_segment, render_position, segments_for_position,
};
