#![recursion_limit = "8192"]
//! Registry-first organization extensions.

// Allow generated code to refer to this crate as `xeno_registry`
extern crate self as xeno_registry;

pub mod core;
pub mod defs;

#[cfg(test)]
mod tests;
// Types re-exported at crate root for `define_events!` macro expansion.
// The macro generates public types (`HookEventData`, `OwnedHookContext`) whose fields
// reference these, so they must be `pub use`.
#[cfg(feature = "hooks")]
pub use domains::hooks::{
	Bool, HookAction, HookResult, Mode, OptionViewId, SplitDirection, Str, ViewId, WindowId,
	WindowKind,
};

#[doc(hidden)]
pub use crate::core as xeno_registry_core;

// Generate HookEvent, HookEventData, OwnedHookContext, and extractor macros
// from this single source of truth. Adding a new event only requires adding
// it here - all extraction machinery is auto-generated.
#[cfg(feature = "hooks")]
xeno_macros::define_events! {
	/// Editor is starting up (before first render).
	EditorStart => "editor:start",
	/// Editor is shutting down.
	EditorQuit => "editor:quit",
	/// Periodic tick.
	EditorTick => "editor:tick",
	/// A buffer was opened/created.
	BufferOpen => "buffer:open" {
		/// Filesystem path of the opened buffer.
		path: Path,
		/// Initial text content of the buffer.
		text: RopeSlice,
		/// Detected file type (e.g., "rust", "python"), if any.
		file_type: OptionStr,
	},
	/// A buffer is about to be written to disk.
	BufferWritePre => "buffer:write-pre" {
		/// Filesystem path where the buffer will be written.
		path: Path,
		/// Buffer content about to be saved.
		text: RopeSlice,
	},
	/// A buffer was written to disk.
	BufferWrite => "buffer:write" {
		/// Filesystem path where the buffer was saved.
		path: Path,
	},
	/// A buffer was closed.
	BufferClose => "buffer:close" {
		/// Filesystem path of the closed buffer.
		path: Path,
		/// File type of the closed buffer, if known.
		file_type: OptionStr,
	},
	/// Buffer content changed.
	BufferChange => "buffer:change" {
		/// Filesystem path of the modified buffer.
		path: Path,
		/// Current text content after the change.
		text: RopeSlice,
		/// File type of the buffer, if known.
		file_type: OptionStr,
		/// Monotonic version number incremented on each change.
		version: u64,
	},
	/// Mode changed (normal -> insert, etc).
	ModeChange => "mode:change" {
		/// Mode before the transition.
		old_mode: Mode,
		/// Mode after the transition.
		new_mode: Mode,
	},
	/// Cursor position changed.
	CursorMove => "cursor:move" {
		/// Zero-based line number of the cursor.
		line: usize,
		/// Zero-based column (grapheme offset) of the cursor.
		col: usize,
	},
	/// Selection changed.
	SelectionChange => "selection:change" {
		/// Byte offset of the selection anchor (start).
		anchor: usize,
		/// Byte offset of the selection head (cursor end).
		head: usize,
	},
	/// Window was resized.
	WindowResize => "window:resize" {
		/// New window width in columns.
		width: u16,
		/// New window height in rows.
		height: u16,
	},
	/// Window gained focus.
	FocusGained => "focus:gained",
	/// Window lost focus.
	FocusLost => "focus:lost",
	/// Focused view changed.
	ViewFocusChanged => "view:focus_changed" {
		/// Identifier of the newly focused view.
		view_id: ViewId,
		/// Identifier of the previously focused view, if any.
		prev_view_id: OptionViewId,
	},
	/// A window was created.
	WindowCreated => "window:created" {
		/// Identifier of the created window.
		window_id: WindowId,
		/// Kind of window created.
		kind: WindowKind,
	},
	/// A window was closed.
	WindowClosed => "window:closed" {
		/// Identifier of the closed window.
		window_id: WindowId,
	},
	/// Focused window changed.
	WindowFocusChanged => "window:focus_changed" {
		/// Identifier of the window whose focus state changed.
		window_id: WindowId,
		/// Whether the window is now focused.
		focused: Bool,
	},
	/// Split view created.
	SplitCreated => "split:created" {
		/// Identifier of the newly created split view.
		view_id: ViewId,
		/// Direction of the split (horizontal or vertical).
		direction: SplitDirection,
	},
	/// Split view closed.
	SplitClosed => "split:closed" {
		/// Identifier of the closed split view.
		view_id: ViewId,
	},
	/// Action execution starting.
	ActionPre => "action:pre" {
		/// Identifier of the action about to execute.
		action_id: Str,
	},
	/// Action execution finished.
	ActionPost => "action:post" {
		/// Identifier of the executed action.
		action_id: Str,
		/// Name of the result variant returned by the action.
		result_variant: Str,
	},
	/// An option value was changed via :set or :setlocal.
	OptionChanged => "option:changed" {
		/// The KDL key of the changed option (e.g., "tab-width").
		key: Str,
		/// The scope of the change: "global" or "buffer".
		scope: Str,
	},
	/// LSP diagnostics were updated for a document.
	DiagnosticsUpdated => "lsp:diagnostics" {
		/// Filesystem path of the document with updated diagnostics.
		path: Path,
		/// Number of error diagnostics.
		error_count: usize,
		/// Number of warning diagnostics.
		warning_count: usize,
	},
}

#[cfg(feature = "db")]
pub mod db;

#[macro_use]
pub mod domains;

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
pub use db::{
	ACTIONS, COMMANDS, GUTTERS, HOOKS, LANGUAGES, LSP_SERVERS, MOTIONS, NOTIFICATIONS, OPTIONS,
	STATUSLINE_SEGMENTS, TEXT_OBJECTS, THEMES,
};
#[cfg(feature = "actions")]
pub use domains::actions;
#[cfg(feature = "commands")]
pub use domains::commands;
#[cfg(feature = "gutter")]
pub use domains::gutter;
#[cfg(feature = "hooks")]
pub use domains::hooks;
#[cfg(feature = "languages")]
pub use domains::languages;
pub use domains::lsp_servers;
#[cfg(feature = "motions")]
pub use domains::motions;
#[cfg(feature = "notifications")]
pub use domains::notifications;
#[cfg(feature = "options")]
pub use domains::options;
#[cfg(feature = "statusline")]
pub use domains::statusline;
#[cfg(feature = "textobj")]
pub use domains::textobj;
#[cfg(feature = "themes")]
pub use domains::themes;
#[cfg(feature = "options")]
pub use xeno_macros::derive_option;

pub use crate::core::*;
