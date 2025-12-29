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
	HookEvent, HookResult, KeyBindingDef, LANGUAGES, LanguageDef, MOTIONS, MUTABLE_HOOKS,
	MotionDef, MutableHookContext, MutableHookDef, OPTIONS, ObjectSelectionKind, OptionDef,
	OptionScope, OptionType, OptionValue, PendingAction, PendingKind, RegistryMetadata,
	RegistrySource, RenderedSegment, ResolvedBinding, STATUSLINE_SEGMENTS, ScrollAmount, ScrollDir,
	SegmentPosition, SegmentStyle, StatuslineContext, StatuslineSegmentDef, TEXT_OBJECTS,
	TextObjectDef, VisualDirection, action, command, dispatch_result, hook, language, motion,
	option, result_handler, statusline_segment, text_object,
};
#[cfg(feature = "host")]
pub use movement::WordType;
pub use notifications::{
	NotifyDEBUGExt, NotifyERRORExt, NotifyINFOExt, NotifySUCCESSExt, NotifyWARNExt,
};
