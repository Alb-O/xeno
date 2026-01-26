//! Async hook system for editor events.
//!
//! Hooks allow extensions to react to editor events like file open, save,
//! mode change, etc.
//!
//! # Async Support
//!
//! Hooks can be either synchronous or asynchronous. The handler function
//! returns a [`HookAction`] which indicates whether the hook completed
//! synchronously or needs async work:
//!
//! ```ignore
//! // Sync hook - completes immediately
//! hook!(my_sync_hook, BufferOpen, 100, "Log buffer opens", |ctx| {
//!     log::info!("Buffer opened");
//!     HookAction::Done
//! });
//!
//! // Async hook - returns a future
//! hook!(my_async_hook, BufferOpen, 100, "Start LSP for buffer", |ctx| {
//!     HookAction::Async(Box::pin(async move {
//!         start_lsp().await;
//!         HookResult::Ok
//!     }))
//! });
//! ```

use std::collections::HashMap;
use std::sync::LazyLock;

pub use xeno_registry_core::{RegistryBuilder, RegistryIndex, RegistryReg, RuntimeRegistry};

mod context;
mod emit;
mod impls;
mod macros;
mod types;

pub use context::{
	Bool, HookContext, MutableHookContext, OptionViewId, SplitDirection, Str, ViewId, WindowId,
	WindowKind,
};
pub use emit::{HookScheduler, emit, emit_mutable, emit_sync, emit_sync_with};
pub use types::{
	BoxFuture, HookAction, HookDef, HookHandler, HookMutability, HookPriority, HookResult,
	RegistryEntry, RegistryMeta, RegistryMetadata, RegistrySource, impl_registry_entry,
};
pub use xeno_primitives::Mode;

/// Registry wrapper for hook definitions.
pub struct HookReg(pub &'static HookDef);
inventory::collect!(HookReg);

impl RegistryReg<HookDef> for HookReg {
	fn def(&self) -> &'static HookDef {
		self.0
	}
}

// Generate HookEvent, HookEventData, OwnedHookContext, and extractor macros
// from this single source of truth. Adding a new event only requires adding
// it here - all extraction machinery is auto-generated.
xeno_macro::define_events! {
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

/// Indexed collection of all hooks with runtime registration support.
pub static HOOKS: LazyLock<RuntimeRegistry<HookDef>> = LazyLock::new(|| {
	let builtins = RegistryBuilder::new("hooks")
		.extend_inventory::<HookReg>()
		.sort_by(|a, b| a.meta.name.cmp(b.meta.name))
		.build();
	RuntimeRegistry::new("hooks", builtins)
});

/// Builtin hooks grouped by event type for efficient dispatch.
///
/// Hooks are sorted by `(priority asc, name asc, id asc)` within each event.
static BUILTIN_BY_EVENT: LazyLock<HashMap<HookEvent, Vec<&'static HookDef>>> =
	LazyLock::new(|| {
		let mut map: HashMap<HookEvent, Vec<&'static HookDef>> = HashMap::new();
		for hook in HOOKS.builtins().iter() {
			map.entry(hook.event).or_default().push(hook);
		}
		// Sort each event's hooks by priority (asc), then name (asc), then id (asc)
		for hooks in map.values_mut() {
			hooks.sort_by(|a, b| {
				a.meta
					.priority
					.cmp(&b.meta.priority)
					.then_with(|| a.meta.name.cmp(b.meta.name))
					.then_with(|| a.meta.id.cmp(b.meta.id))
			});
		}
		map
	});

/// Registers a new hook definition at runtime.
///
/// Returns `true` if the hook was successfully added, or `false` if a hook with
/// the same identity is already registered (either as a builtin or a previous
/// runtime addition).
///
/// # Panics
///
/// Panics if the hook violates registry invariants (e.g., name shadows an ID).
pub fn register_hook(def: &'static HookDef) -> bool {
	HOOKS.register(def)
}

/// Returns all hooks registered for the given `event`, in execution order.
///
/// The execution order is strictly deterministic and follows the registry's
/// global total order (Priority, Source, then ID). Both builtin and runtime-added
/// hooks are included.
pub fn hooks_for_event(event: HookEvent) -> Vec<&'static HookDef> {
	let mut hooks: Vec<_> = BUILTIN_BY_EVENT
		.get(&event)
		.map(Vec::as_slice)
		.unwrap_or(&[])
		.to_vec();

	// Integrate runtime extensions matching this event
	hooks.extend(
		HOOKS
			.extras_items()
			.into_iter()
			.filter(|h| h.event == event),
	);

	// Ensure global consistency across builtin and runtime hooks
	hooks.sort_by(|a, b| a.total_order_cmp(b));

	hooks
}

/// Find all hooks registered for a specific event.
pub fn find_hooks(event: HookEvent) -> impl Iterator<Item = &'static HookDef> {
	hooks_for_event(event).into_iter()
}

/// List all registered hooks (builtins + runtime).
pub fn all_hooks() -> impl Iterator<Item = &'static HookDef> {
	HOOKS.all().into_iter()
}
