#![recursion_limit = "8192"]
//! Registry-first organization extensions.

// Allow generated code to refer to this crate as `xeno_registry`
extern crate self as xeno_registry;

// Re-export core crate for macros
pub use xeno_registry_core;

pub mod inventory;

#[cfg(feature = "db")]
pub mod db;

#[cfg(feature = "actions")]
pub mod actions;
#[cfg(feature = "commands")]
pub mod commands;
#[cfg(feature = "gutter")]
pub mod gutter;
#[cfg(feature = "hooks")]
pub mod hooks;
#[cfg(feature = "motions")]
pub mod motions;
#[cfg(feature = "notifications")]
pub mod notifications;
#[cfg(feature = "options")]
pub mod options;
#[cfg(feature = "statusline")]
pub mod statusline;
#[cfg(feature = "textobj")]
pub mod textobj;
#[cfg(feature = "themes")]
pub mod themes;

// Centralized inventory collection
#[cfg(feature = "actions")]
crate::inventory::collect!(crate::actions::ActionDef);
#[cfg(feature = "actions")]
crate::inventory::collect_slice!(crate::actions::KeyBindingDef);

#[cfg(feature = "commands")]
crate::inventory::collect!(crate::commands::CommandDef);

#[cfg(feature = "motions")]
crate::inventory::collect!(crate::motions::MotionDef);

#[cfg(feature = "textobj")]
crate::inventory::collect!(crate::textobj::TextObjectDef);

#[cfg(feature = "options")]
crate::inventory::collect!(crate::options::OptionDef);

#[cfg(feature = "themes")]
crate::inventory::collect!(crate::themes::theme::ThemeDef);

#[cfg(feature = "statusline")]
crate::inventory::collect!(crate::statusline::StatuslineSegmentDef);

#[cfg(feature = "gutter")]
crate::inventory::collect!(crate::gutter::GutterDef);

#[cfg(feature = "hooks")]
crate::inventory::collect!(crate::hooks::HookDef);

#[cfg(feature = "notifications")]
crate::inventory::collect!(crate::notifications::NotificationDef);

// Re-exports for convenience
#[cfg(feature = "actions")]
pub use actions::editor_ctx::{
	CommandQueueAccess, CursorAccess, EditAccess, EditorCapabilities, EditorContext, EditorOps,
	FileOpsAccess, FocusOps, HandleOutcome, JumpAccess, MacroAccess, ModeAccess, MotionAccess,
	MotionDispatchAccess, NotificationAccess, OptionAccess, PaletteAccess, ResultHandler,
	SearchAccess, SelectionAccess, SplitOps, TextAccess, ThemeAccess, UndoAccess, ViewportAccess,
};
#[cfg(feature = "actions")]
pub use actions::{
	ActionArgs, ActionContext, ActionDef, ActionEffects, ActionHandler, ActionResult, AppEffect,
	BindingMode, EditEffect, Effect, Mode, MotionKind, MotionRequest, ObjectSelectionKind,
	PendingAction, PendingKind, RESULT_EFFECTS_HANDLERS, RESULT_EXTENSION_HANDLERS,
	ResultHandlerRegistry, ScreenPosition, ScrollAmount, UiEffect, ViewEffect, dispatch_result,
	edit_op, find_prefix,
};
#[cfg(feature = "actions")]
pub use actions::{Axis, SeqDirection, SpatialDirection};
#[cfg(feature = "commands")]
pub use commands::{
	CommandContext, CommandDef, CommandEditorOps, CommandError, CommandHandler, CommandOutcome,
	CommandResult,
};
#[cfg(feature = "db")]
pub use db::index;
#[cfg(feature = "db")]
pub use db::index::{
	all_actions, all_commands, all_motions, all_text_objects, find_action, find_action_by_id,
	find_command, find_motion, find_text_object_by_trigger, resolve_action_id, resolve_action_key,
};
#[cfg(feature = "keymap")]
pub use db::keymap_registry::{BindingEntry, KeymapRegistry, LookupResult, get_keymap_registry};
#[cfg(feature = "db")]
pub use db::plugin::XenoPlugin;
#[cfg(feature = "db")]
pub use db::{
	ACTIONS, COMMANDS, GUTTERS, HOOKS, MOTIONS, NOTIFICATIONS, OPTIONS, STATUSLINE_SEGMENTS,
	TEXT_OBJECTS, THEMES,
};
#[cfg(feature = "gutter")]
pub use gutter::{
	GutterAnnotations, GutterCell, GutterDef, GutterLineContext, GutterSegment, GutterWidth,
	GutterWidthContext,
};
#[cfg(feature = "hooks")]
pub use hooks::{
	Bool, BoxFuture, HookAction, HookContext, HookDef, HookEvent, HookEventData, HookHandler,
	HookMutability, HookPriority, HookResult, HookScheduler, MutableHookContext, OptionViewId,
	OwnedHookContext, SplitDirection, Str, ViewId, WindowId, WindowKind, emit, emit_mutable,
	emit_sync, emit_sync_with,
};
#[cfg(feature = "motions")]
pub use motions::{Capability, MotionDef, MotionHandler, flags, movement};
#[cfg(feature = "notifications")]
pub use notifications::{
	AutoDismiss, IntoNotification, Level, Notification, NotificationDef, NotificationKey,
	keys as notification_keys,
};
#[cfg(feature = "options")]
pub use options::{
	OptionDef, OptionError, OptionKey, OptionReg, OptionScope, OptionType, OptionValidator,
	OptionValue, TypedOptionKey, validate,
};
#[cfg(feature = "statusline")]
pub use statusline::{
	RenderedSegment, SegmentPosition, SegmentStyle, StatuslineContext, StatuslineSegmentDef,
	render_position,
};
#[cfg(feature = "textobj")]
pub use textobj::{TextObjectDef, TextObjectHandler};
pub use xeno_registry_core::{ActionId, RegistryMetadata, RegistrySource};
