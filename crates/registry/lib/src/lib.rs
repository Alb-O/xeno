//! Registry-first organization extensions.
//!
//! This crate aggregates all registry sub-crates. Depend on this crate to get
//! access to all registries, rather than depending on individual registry crates.
//!
//! # Sub-crates
//!
//! - [`gutter`] - Gutter columns (line numbers, signs, diagnostics)
//! - [`motions`] - Cursor movement primitives
//! - [`options`] - Configuration options
//! - [`notifications`] - Notification types
//! - [`commands`] - Ex-mode command definitions
//! - [`actions`] - Action definitions and handlers
//! - [`hooks`] - Event lifecycle observers
//! - [`statusline`] - Statusline segment definitions
//! - [`textobj`] - Text object selection (inner/around)
//!
//! # Infrastructure
//!
//! - [`index`] - Registry indexing and lookup
//! - [`keymap_registry`] - Keybinding resolution
//!
//! # Adding a New Registry
//!
//! 1. Create `crates/registry/{name}/` with Cargo.toml and src/
//! 2. Add to root `Cargo.toml` members and workspace.dependencies
//! 3. Add dependency and re-export here

/// Registry indexing and lookup for editor extensions.
pub mod index;
/// Unified keymap registry using trie-based matching.
pub mod keymap_registry;

// Re-export commonly used items at the crate root for convenience
// Data-oriented edit operations
pub use actions::editor_ctx::{
	CommandQueueAccess, CursorAccess, EditAccess, EditorCapabilities, EditorContext, EditorOps,
	FileOpsAccess, FocusOps, HandleOutcome, JumpAccess, MacroAccess, ModeAccess,
	NotificationAccess, OptionAccess, PaletteAccess, ResultHandler, SearchAccess, SelectionAccess,
	SplitOps, TextAccess, ThemeAccess, UndoAccess, ViewportAccess,
};
pub use actions::{
	ACTIONS, ActionArgs, ActionContext, ActionDef, ActionEffects, ActionHandler, ActionResult,
	BindingMode, Effect, KEY_PREFIXES, KEYBINDINGS, KeyBindingDef, KeyPrefixDef, Mode,
	ObjectSelectionKind, PendingAction, PendingKind, RESULT_EFFECTS_HANDLERS,
	RESULT_EXTENSION_HANDLERS, ScreenPosition, ScrollAmount, action, dispatch_result, edit_op,
	find_prefix, key_prefix, result_extension_handler, result_handler,
};
// Re-export direction types (via actions which re-exports from xeno-base)
pub use actions::{Axis, SeqDirection, SpatialDirection};
pub use commands::{
	COMMANDS, CommandContext, CommandDef, CommandEditorOps, CommandError, CommandHandler,
	CommandOutcome, CommandResult, all_commands, command, find_command,
};
pub use gutter::{
	GUTTERS, GutterAnnotations, GutterCell, GutterDef, GutterLineContext, GutterStyle, GutterWidth,
	GutterWidthContext, all as all_gutters, column_width, column_widths, enabled_gutters,
	find as find_gutter, gutter, total_width as gutter_total_width,
};
pub use hooks::{
	Bool, BoxFuture, HOOKS, HookAction, HookContext, HookDef, HookEvent, HookEventData,
	HookHandler, HookMutability, HookResult, HookScheduler, MutableHookContext, OptionViewId,
	OwnedHookContext, SplitDirection, Str, ViewId, WindowId, WindowKind, all_hooks, async_hook,
	emit, emit_mutable, emit_sync, emit_sync_with, find_hooks, hook,
};
// Re-export index lookup functions (excluding duplicates from commands)
pub use index::{
	all_actions, all_motions, all_text_objects, find_action, find_action_by_id, find_motion,
	find_text_object_by_trigger, resolve_action_id, resolve_action_key,
};
// Re-export keymap registry
pub use keymap_registry::{BindingEntry, KeymapRegistry, LookupResult, get_keymap_registry};
pub use motions::{Capability, MOTIONS, MotionDef, MotionHandler, flags, motion, movement};
pub use notifications::{
	AutoDismiss, IntoNotification, Level, NOTIFICATIONS, Notification, NotificationDef,
	NotificationKey, keys as notification_keys,
};
pub use statusline::{
	RenderedSegment, STATUSLINE_SEGMENTS, SegmentPosition, SegmentStyle, StatuslineContext,
	StatuslineSegmentDef, all_segments, find_segment, render_position, segments_for_position,
	statusline_segment,
};
pub use textobj::{
	TEXT_OBJECTS, TextObjectDef, TextObjectHandler, bracket_pair_object, symmetric_text_object,
	text_object,
};
// Re-export shared types from registry core (canonical source)
pub use xeno_registry_core::{ActionId, RegistryMetadata, RegistrySource, impl_registry_metadata};
pub use {
	xeno_registry_actions as actions, xeno_registry_commands as commands,
	xeno_registry_gutter as gutter, xeno_registry_hooks as hooks, xeno_registry_motions as motions,
	xeno_registry_notifications as notifications, xeno_registry_options as options,
	xeno_registry_statusline as statusline, xeno_registry_textobj as textobj,
	xeno_registry_themes as themes,
};
