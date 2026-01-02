//! Standard library of built-in implementations for evildoer.
//!
//! This crate contains the concrete implementations of actions, commands, motions,
//! text objects, hooks, options, and statusline segments.
//!
//! Note: Language/filetype definitions are loaded at runtime from `languages.kdl`
//! via the `evildoer-language` crate.

pub mod actions;
pub mod commands;
pub mod editor_ctx;
pub mod hooks;
pub mod motions;
#[cfg(feature = "host")]
pub mod movement;
pub mod notifications;
pub mod objects;
pub mod options;
pub mod statusline;

pub use evildoer_base::range::CharIdx;
pub use evildoer_base::{
	ChangeSet, Key, KeyCode, Modifiers, MouseButton, MouseEvent, Range, Rope, RopeSlice,
	ScrollDirection, Selection, Transaction,
};
pub use evildoer_manifest::{
	ACTIONS, ActionArgs, ActionContext, ActionDef, ActionHandler, ActionMode, ActionResult,
	BindingMode, COMMANDS, Capability, CommandContext, CommandDef, CommandError, CommandOutcome,
	CommandResult, CompletionContext, CompletionItem, CompletionKind, CompletionSource, EditAction,
	EditorCapabilities, EditorContext, EditorOps, HOOKS, HandleOutcome, HookContext, HookDef,
	HookEvent, HookHandler, HookMutability, HookResult, KeyBindingDef, MOTIONS, MotionDef,
	MutableHookContext, OPTIONS, ObjectSelectionKind, OptionDef, OptionScope, OptionType,
	OptionValue, PendingAction, PendingKind, RegistryMetadata, RegistrySource, RenderedSegment,
	STATUSLINE_SEGMENTS, ScrollAmount, ScrollDir, SegmentPosition, SegmentStyle, StatuslineContext,
	StatuslineSegmentDef, TEXT_OBJECTS, TextObjectDef, VisualDirection, action, async_hook,
	bracket_pair_object, command, dispatch_result, hook, motion, option, result_handler,
	statusline_segment, symmetric_text_object, text_object,
};
#[cfg(feature = "host")]
pub use movement::WordType;
pub use notifications::{
	NotifyDEBUGExt, NotifyERRORExt, NotifyINFOExt, NotifySUCCESSExt, NotifyWARNExt,
};
