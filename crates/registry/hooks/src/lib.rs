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

pub use xeno_registry_core::{RegistryBuilder, RegistryIndex, RegistryReg};

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

/// Indexed collection of all builtin hooks.
///
/// # Event-Indexed Pattern
///
/// Unlike actions/commands which use [`RuntimeRegistry`] for name-based lookup,
/// hooks use a different pattern because dispatch is by [`HookEvent`] enum, not
/// by string name. The registry is split into three components:
///
/// | Component | Type | Purpose |
/// |-----------|------|---------|
/// | [`HOOKS`] | [`RegistryIndex`] | Name/ID lookup, iteration, introspection |
/// | `BUILTIN_BY_EVENT` | `HashMap` | Compile-time hook dispatch by event |
/// | `EXTRA_BY_EVENT` | `RwLock<HashMap>` | Runtime hook dispatch by event |
///
/// This separation is intentional: [`RegistryIndex`] handles identity and
/// introspection, while the event maps handle efficient dispatch.
///
/// # Ordering Guarantees
///
/// Hooks within each event are sorted by `(priority asc, name asc)`:
/// - Lower priority numbers run first
/// - Name provides stable tie-breaking
///
/// This matches the emit behavior in [`emit`](crate::emit).
///
/// # Future Considerations
///
/// ## Index-Based References
///
/// Current implementation stores `Vec<&'static HookDef>` in event maps.
/// An alternative is storing indices into [`HOOKS`]:
///
/// ```ignore
/// pub struct HookIndex {
///     pub hooks: RegistryIndex<HookDef>,
///     pub by_event: HashMap<HookEvent, Vec<usize>>,
/// }
/// ```
///
/// Benefits: smaller memory footprint, no lifetime gymnastics, single source of truth.
/// Tradeoffs: indirect lookup, more complex runtime registration.
///
/// Current approach is fine given hook count (~10 total). Reconsider if:
/// - Hook count grows significantly (100+)
/// - Memory pressure becomes a concern
/// - Need to support hook removal
///
/// ## Array-Based Event Index
///
/// If [`HookEvent`] remains a small enum (~20 variants), consider:
///
/// ```ignore
/// struct EventIndex {
///     // Direct indexing, no hash lookup
///     by_event: [Vec<&'static HookDef>; HookEvent::COUNT],
/// }
/// ```
///
/// Requires [`HookEvent`] to implement `as_usize()` and `COUNT`.
///
/// [`RuntimeRegistry`]: xeno_registry_core::RuntimeRegistry
pub static HOOKS: LazyLock<RegistryIndex<HookDef>> = LazyLock::new(|| {
	RegistryBuilder::new("hooks")
		.extend_inventory::<HookReg>()
		.sort_by(|a, b| a.meta.name.cmp(b.meta.name))
		.build()
});

/// Builtin hooks grouped by event type for efficient dispatch.
///
/// Hooks are sorted by `(priority asc, name asc)` within each event.
/// See [`HOOKS`] for architectural rationale.
static BUILTIN_BY_EVENT: LazyLock<HashMap<HookEvent, Vec<&'static HookDef>>> =
	LazyLock::new(|| {
		let mut map: HashMap<HookEvent, Vec<&'static HookDef>> = HashMap::new();
		for hook in HOOKS.iter() {
			map.entry(hook.event).or_default().push(hook);
		}
		// Sort each event's hooks by priority (asc), then name (asc)
		for hooks in map.values_mut() {
			hooks.sort_by(|a, b| {
				a.meta
					.priority
					.cmp(&b.meta.priority)
					.then_with(|| a.meta.name.cmp(b.meta.name))
			});
		}
		map
	});

/// Runtime-registered hooks grouped by event type.
///
/// Separated from [`BUILTIN_BY_EVENT`] to allow runtime registration without
/// rebuilding the entire index. See [`HOOKS`] for architectural rationale.
static EXTRA_BY_EVENT: LazyLock<std::sync::RwLock<HashMap<HookEvent, Vec<&'static HookDef>>>> =
	LazyLock::new(|| std::sync::RwLock::new(HashMap::new()));

/// Registers an extra hook definition at runtime.
///
/// Returns `true` if the hook was added, `false` if already registered.
/// Maintains sorted order `(priority asc, name asc)` within each event
/// via `binary_search_by()` + `insert()`.
///
/// # Future: Batch Registration
///
/// If plugin registration becomes batch-oriented, consider:
///
/// ```ignore
/// pub fn register_hooks_batch(defs: &[&'static HookDef]) {
///     // Add all to extras, rebuild by_event from scratch
///     // More efficient than N sorted inserts
/// }
/// ```
pub fn register_hook(def: &'static HookDef) -> bool {
	if HOOKS.items().iter().any(|&h| std::ptr::eq(h, def)) {
		return false;
	}

	let mut extras = EXTRA_BY_EVENT.write().expect("poisoned");
	let event_hooks = extras.entry(def.event).or_default();

	if event_hooks.iter().any(|&h| std::ptr::eq(h, def)) {
		return false;
	}

	// Insert in sorted position (priority asc, name asc)
	let pos = event_hooks
		.binary_search_by(|h| {
			h.meta
				.priority
				.cmp(&def.meta.priority)
				.then_with(|| h.meta.name.cmp(def.meta.name))
		})
		.unwrap_or_else(|i| i);
	event_hooks.insert(pos, def);
	true
}

/// Returns hooks matching the given event, including runtime registrations.
pub fn hooks_for_event(event: HookEvent) -> Vec<&'static HookDef> {
	let builtins = BUILTIN_BY_EVENT
		.get(&event)
		.map(Vec::as_slice)
		.unwrap_or(&[]);
	let extras_guard = EXTRA_BY_EVENT.read().expect("poisoned");
	let extras = extras_guard.get(&event).map(Vec::as_slice).unwrap_or(&[]);

	builtins
		.iter()
		.copied()
		.chain(extras.iter().copied())
		.collect()
}

/// Find all hooks registered for a specific event.
pub fn find_hooks(event: HookEvent) -> impl Iterator<Item = &'static HookDef> {
	hooks_for_event(event).into_iter()
}

/// List all registered hooks (builtins + runtime).
pub fn all_hooks() -> impl Iterator<Item = &'static HookDef> {
	let mut hooks: Vec<_> = HOOKS.items().to_vec();
	let extras = EXTRA_BY_EVENT.read().expect("poisoned");
	for event_hooks in extras.values() {
		hooks.extend(event_hooks.iter().copied());
	}
	hooks.into_iter()
}
