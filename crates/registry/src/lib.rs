//! Registry-first organization extensions.
//!
//! This crate aggregates all registry sub-crates. Depend on this crate to get
//! access to all registries, rather than depending on individual registry crates.
//!
//! # Sub-crates
//!
//! - [`gutter`] - Gutter columns (line numbers, signs, diagnostics)
//! - [`menus`] - Menu bar groups and items
//! - [`motions`] - Cursor movement primitives
//! - [`options`] - Configuration options
//! - [`notifications`] - Notification types
//! - [`commands`] - Ex-mode command definitions
//! - [`actions`] - Action definitions and handlers
//! - [`hooks`] - Event lifecycle observers
//! - [`statusline`] - Statusline segment definitions
//! - [`text_objects`] - Text object selection (inner/around)
//!
//! # Adding a New Registry
//!
//! 1. Create `crates/registry/{name}/` with Cargo.toml and src/
//! 2. Add to root `Cargo.toml` members and workspace.dependencies
//! 3. Add dependency and re-export here

// Re-export commonly used items at the crate root for convenience
pub use actions::editor_ctx::{
	CommandQueueAccess, CursorAccess, EditAccess, EditorCapabilities, EditorContext, EditorOps,
	FileOpsAccess, FocusOps, HandleOutcome, JumpAccess, MacroAccess, NotificationAccess,
	ResultHandler, SearchAccess, SelectionAccess, SplitOps, TextAccess, ThemeAccess, UndoAccess,
};
pub use actions::{
	ACTIONS, ActionArgs, ActionContext, ActionDef, ActionHandler, ActionMode, ActionResult,
	BindingMode, EditAction, KEY_PREFIXES, KEYBINDINGS, KeyBindingDef, KeyPrefixDef, Mode,
	ObjectSelectionKind, PendingAction, PendingKind, RESULT_BUFFER_NEXT_HANDLERS,
	RESULT_BUFFER_PREV_HANDLERS, RESULT_CLOSE_OTHER_BUFFERS_HANDLERS,
	RESULT_CLOSE_PALETTE_HANDLERS, RESULT_CLOSE_SPLIT_HANDLERS, RESULT_COMMAND_HANDLERS,
	RESULT_CURSOR_MOVE_HANDLERS, RESULT_EDIT_HANDLERS, RESULT_ERROR_HANDLERS,
	RESULT_EXECUTE_PALETTE_HANDLERS, RESULT_EXTENSION_HANDLERS, RESULT_FOCUS_DOWN_HANDLERS,
	RESULT_FOCUS_LEFT_HANDLERS, RESULT_FOCUS_RIGHT_HANDLERS, RESULT_FOCUS_UP_HANDLERS,
	RESULT_FORCE_REDRAW_HANDLERS, RESULT_INSERT_WITH_MOTION_HANDLERS, RESULT_MODE_CHANGE_HANDLERS,
	RESULT_MOTION_HANDLERS, RESULT_OK_HANDLERS, RESULT_OPEN_PALETTE_HANDLERS,
	RESULT_PENDING_HANDLERS, RESULT_QUIT_HANDLERS, RESULT_SCREEN_MOTION_HANDLERS,
	RESULT_SEARCH_NEXT_HANDLERS, RESULT_SEARCH_PREV_HANDLERS, RESULT_SPLIT_HORIZONTAL_HANDLERS,
	RESULT_SPLIT_VERTICAL_HANDLERS, RESULT_USE_SELECTION_SEARCH_HANDLERS, ScrollAmount, ScrollDir,
	VisualDirection, action, dispatch_result, find_prefix, key_prefix, result_extension_handler,
	result_handler,
};
pub use commands::{
	COMMANDS, CommandContext, CommandDef, CommandEditorOps, CommandError, CommandHandler,
	CommandOutcome, CommandResult, all_commands, command, find_command,
};
pub use hooks::{
	Bool, BoxFuture, HOOKS, HookAction, HookContext, HookDef, HookEvent, HookEventData,
	HookHandler, HookMutability, HookResult, HookScheduler, MutableHookContext, OptionViewId,
	OwnedHookContext, SplitDirection, Str, ViewId, WindowId, WindowKind, all_hooks, async_hook,
	emit, emit_mutable, emit_sync, emit_sync_with, find_hooks, hook,
};
pub use menus::{MENU_GROUPS, MENU_ITEMS, MenuGroupDef, MenuItemDef, menu_group, menu_item};
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
pub use text_objects::{
	TEXT_OBJECTS, TextObjectDef, TextObjectHandler, bracket_pair_object, symmetric_text_object,
	text_object,
};
// Re-export shared types from registry core (canonical source)
pub use xeno_registry_core::{RegistryMetadata, RegistrySource, impl_registry_metadata};
pub use gutter::{
	GUTTERS, GutterAnnotations, GutterCell, GutterDef, GutterLineContext, GutterStyle,
	GutterWidth, GutterWidthContext, GitHunkStatus, all as all_gutters, column_width,
	column_widths, enabled_gutters, find as find_gutter, gutter, total_width as gutter_total_width,
};
pub use xeno_registry_options::option;
pub use {
	xeno_registry_actions as actions, xeno_registry_commands as commands,
	xeno_registry_gutter as gutter, xeno_registry_hooks as hooks, xeno_registry_menus as menus,
	xeno_registry_motions as motions, xeno_registry_notifications as notifications,
	xeno_registry_options as options, xeno_registry_statusline as statusline,
	xeno_registry_text_objects as text_objects, xeno_registry_themes as themes,
};
