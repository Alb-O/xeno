//! Standard library of built-in implementations for evildoer.
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
mod window_actions;

// Re-export types from evildoer-manifest for convenience
// Re-export WordType from movement when host feature is enabled
#[cfg(feature = "host")]
pub use movement::WordType;
// Re-export notification extension traits for convenient use
pub use notifications::{
	NotifyDEBUGExt, NotifyERRORExt, NotifyINFOExt, NotifySUCCESSExt, NotifyWARNExt,
};
// Re-export types from evildoer-base for convenience
pub use evildoer_base::range::CharIdx;
pub use evildoer_base::{
	ChangeSet, Key, KeyCode, Modifiers, MouseButton, MouseEvent, Range, Rope, RopeSlice,
	ScrollDirection, Selection, SpecialKey, Transaction,
};
pub use evildoer_manifest::{
	ACTIONS, ActionArgs, ActionContext, ActionDef, ActionHandler, ActionMode, ActionResult,
	BindingMode, COMMANDS, Capability, CommandContext, CommandDef, CommandError, CommandOutcome,
	CommandResult, CompletionContext, CompletionItem, CompletionKind, CompletionSource, EditAction,
	EditorCapabilities, EditorContext, EditorOps, HOOKS, HandleOutcome, HookContext, HookDef,
	HookEvent, HookResult, KeyBindingDef, LANGUAGES, LanguageDef, MOTIONS, MUTABLE_HOOKS,
	MotionDef, MutableHookContext, MutableHookDef, OPTIONS, ObjectSelectionKind, OptionDef,
	OptionScope, OptionType, OptionValue, PendingAction, PendingKind, RegistryMetadata,
	RegistrySource, RenderedSegment, ResolvedBinding, STATUSLINE_SEGMENTS, ScrollAmount, ScrollDir,
	SegmentPosition, SegmentStyle, StatuslineContext, StatuslineSegmentDef, TEXT_OBJECTS,
	TextObjectDef, VisualDirection, dispatch_result,
};
// Re-export macros from evildoer-manifest
pub use evildoer_manifest::{action, command, hook, language, motion, option, text_object};
