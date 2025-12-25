//! Standard library of built-in implementations for tome.
//!
//! This crate contains the concrete implementations of actions, commands, motions,
//! text objects, filetypes, hooks, options, and statusline segments.

pub mod actions;
pub mod commands;
pub mod editor_ctx;
pub mod filetypes;
pub mod hooks;
pub mod motions;
#[cfg(feature = "host")]
pub mod movement;
pub mod notifications;
pub mod objects;
pub mod options;
pub mod statusline;

// Re-export types from tome-manifest for convenience
// Re-export WordType from movement when host feature is enabled
#[cfg(feature = "host")]
pub use movement::WordType;
// Re-export types from tome-base for convenience
pub use tome_base::range::CharIdx;
pub use tome_base::{
	ChangeSet, Key, KeyCode, Modifiers, MouseButton, MouseEvent, Range, Rope, RopeSlice,
	ScrollDirection, Selection, SpecialKey, Transaction,
};
pub use tome_manifest::{
	ACTIONS, ActionArgs, ActionContext, ActionDef, ActionHandler, ActionMode, ActionResult,
	BindingMode, COMMANDS, Capability, CommandContext, CommandDef, CommandError, CommandOutcome,
	CommandResult, CompletionContext, CompletionItem, CompletionKind, CompletionSource, EditAction,
	EditorCapabilities, EditorContext, EditorOps, FILE_TYPES, FileTypeDef, HOOKS, HandleOutcome,
	HookContext, HookDef, HookEvent, HookResult, KeyBindingDef, MOTIONS, MUTABLE_HOOKS, MotionDef,
	MutableHookContext, MutableHookDef, OPTIONS, ObjectSelectionKind, OptionDef, OptionScope,
	OptionType, OptionValue, PendingAction, PendingKind, RegistryMetadata, RegistrySource,
	RenderedSegment, ResolvedBinding, STATUSLINE_SEGMENTS, ScrollAmount, ScrollDir,
	SegmentPosition, SegmentStyle, StatuslineContext, StatuslineSegmentDef, TEXT_OBJECTS,
	TextObjectDef, VisualDirection, dispatch_result,
};
// Re-export macros from tome-manifest
pub use tome_manifest::{action, command, filetype, hook, motion, option, text_object};
