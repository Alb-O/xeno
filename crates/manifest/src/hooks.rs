//! Async hook system for editor events.
//!
//! Hooks allow extensions to react to editor events like file open, save,
//! mode change, etc. They are registered at compile-time using `linkme`.
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
//!         lsp_manager.on_buffer_open(path).await;
//!         HookResult::Continue
//!     }))
//! });
//! ```

use std::any::Any;
use std::path::Path;
use std::pin::Pin;

use futures::future::Future;
use linkme::distributed_slice;
use tracing::warn;

use crate::{Mode, PanelId, RegistrySource};

/// Identifier for a focused view in hook payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewId {
	/// A text buffer view, identified by its buffer ID.
	Text(u64),
	/// A panel view, identified by its panel ID.
	Panel(PanelId),
}

/// Optional view identifier for hook payloads.
pub type OptionViewId = Option<ViewId>;

/// Static string payload for hook events.
pub type Str = &'static str;

/// Boolean payload for hook events.
pub type Bool = bool;

/// Split direction for layout-related events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
	/// Horizontal split (side-by-side).
	Horizontal,
	/// Vertical split (stacked).
	Vertical,
}

// Generate HookEvent, HookEventData, OwnedHookContext, and extractor macros
// from this single source of truth. Adding a new event only requires adding
// it here - all extraction machinery is auto-generated.
evildoer_macro::define_events! {
	/// Editor is starting up (before first render).
	EditorStart => "editor:start",
	/// Editor is shutting down.
	EditorQuit => "editor:quit",
	/// Periodic tick.
	EditorTick => "editor:tick",
	/// A buffer was opened/created.
	BufferOpen => "buffer:open" {
		path: Path,
		text: RopeSlice,
		file_type: OptionStr,
	},
	/// A buffer is about to be written to disk.
	BufferWritePre => "buffer:write-pre" {
		path: Path,
		text: RopeSlice,
	},
	/// A buffer was written to disk.
	BufferWrite => "buffer:write" {
		path: Path,
	},
	/// A buffer was closed.
	BufferClose => "buffer:close" {
		path: Path,
		file_type: OptionStr,
	},
	/// Buffer content changed.
	BufferChange => "buffer:change" {
		path: Path,
		text: RopeSlice,
		file_type: OptionStr,
		version: u64,
	},
	/// Mode changed (normal -> insert, etc).
	ModeChange => "mode:change" {
		old_mode: Mode,
		new_mode: Mode,
	},
	/// Cursor position changed.
	CursorMove => "cursor:move" {
		line: usize,
		col: usize,
	},
	/// Selection changed.
	SelectionChange => "selection:change" {
		anchor: usize,
		head: usize,
	},
	/// Window was resized.
	WindowResize => "window:resize" {
		width: u16,
		height: u16,
	},
	/// Window gained focus.
	FocusGained => "focus:gained",
	/// Window lost focus.
	FocusLost => "focus:lost",
	/// Focused view changed.
	ViewFocusChanged => "view:focus_changed" {
		view_id: ViewId,
		prev_view_id: OptionViewId,
	},
	/// Split view created.
	SplitCreated => "split:created" {
		view_id: ViewId,
		direction: SplitDirection,
	},
	/// Split view closed.
	SplitClosed => "split:closed" {
		view_id: ViewId,
	},
	/// Panel visibility toggled.
	PanelToggled => "panel:toggled" {
		panel_id: Str,
		visible: Bool,
	},
	/// Action execution starting.
	ActionPre => "action:pre" {
		action_id: Str,
	},
	/// Action execution finished.
	ActionPost => "action:post" {
		action_id: Str,
		result_variant: Str,
	},
}

/// Context passed to hook handlers.
///
/// Contains event-specific data plus type-erased access to extension services.
/// For hooks that need to modify state, use [`MutableHookContext`].
pub struct HookContext<'a> {
	/// The event-specific data.
	pub data: HookEventData<'a>,
	/// Type-erased access to `ExtensionMap` (from `evildoer-api`).
	extensions: Option<&'a dyn Any>,
}

impl<'a> HookContext<'a> {
	/// Creates a new hook context with event data and optional extensions.
	pub fn new(data: HookEventData<'a>, extensions: Option<&'a dyn Any>) -> Self {
		Self { data, extensions }
	}

	/// Returns the event type for this context.
	pub fn event(&self) -> HookEvent {
		self.data.event()
	}

	/// Creates an owned version of the event data for use in async hooks.
	///
	/// Async hooks must extract extension handles separately before returning a future.
	pub fn to_owned(&self) -> OwnedHookContext {
		self.data.to_owned()
	}

	/// Attempts to downcast the extensions to a concrete type.
	///
	/// Used to access `ExtensionMap` from `evildoer-api` without creating a dependency.
	pub fn extensions<T: Any>(&self) -> Option<&'a T> {
		self.extensions?.downcast_ref::<T>()
	}
}

/// Result of a hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HookResult {
	/// Continue with the operation.
	#[default]
	Continue,
	/// Cancel the operation (for pre-hooks like BufferWritePre).
	Cancel,
}

/// A boxed future that returns a [`HookResult`].
pub type BoxFuture = Pin<Box<dyn Future<Output = HookResult> + Send + 'static>>;

/// Action returned by a hook handler.
///
/// Hooks return this to indicate whether they completed synchronously
/// or need async work.
pub enum HookAction {
	/// Hook completed synchronously with the given result.
	Done(HookResult),
	/// Hook needs async work. The future will be awaited.
	Async(BoxFuture),
}

impl HookAction {
	/// Create a sync action that continues.
	pub fn done() -> Self {
		HookAction::Done(HookResult::Continue)
	}

	/// Create a sync action that cancels.
	pub fn cancel() -> Self {
		HookAction::Done(HookResult::Cancel)
	}
}

impl From<HookResult> for HookAction {
	fn from(result: HookResult) -> Self {
		HookAction::Done(result)
	}
}

impl From<()> for HookAction {
	fn from(_: ()) -> Self {
		HookAction::Done(HookResult::Continue)
	}
}

/// Whether a hook can mutate editor state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookMutability {
	Immutable,
	Mutable,
}

/// Handler function for a hook.
#[derive(Clone, Copy)]
pub enum HookHandler {
	Immutable(fn(&HookContext) -> HookAction),
	Mutable(fn(&mut MutableHookContext) -> HookAction),
}

/// A hook that responds to editor events.
#[derive(Clone, Copy)]
pub struct HookDef {
	/// Unique identifier.
	pub id: &'static str,
	/// Hook name for debugging/logging.
	pub name: &'static str,
	/// The event this hook responds to.
	pub event: HookEvent,
	/// Short description.
	pub description: &'static str,
	/// Priority (lower runs first, default 100).
	pub priority: i16,
	/// Whether this hook can mutate editor state.
	pub mutability: HookMutability,
	/// The hook handler function.
	///
	/// Returns [`HookAction::Done`] for sync completion or [`HookAction::Async`]
	/// with a future for async work.
	pub handler: HookHandler,
	/// Origin of the hook.
	pub source: RegistrySource,
}

impl std::fmt::Debug for HookDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("HookDef")
			.field("name", &self.name)
			.field("event", &self.event)
			.field("mutability", &self.mutability)
			.field("priority", &self.priority)
			.field("description", &self.description)
			.finish()
	}
}

crate::impl_registry_metadata!(HookDef);

/// Mutable context passed to mutable hook handlers.
pub struct MutableHookContext<'a> {
	/// The event being processed.
	pub event: HookEvent,
	/// Mutable document content (if applicable).
	pub text: Option<&'a mut ropey::Rope>,
	/// File path (if applicable).
	pub path: Option<&'a Path>,
	/// File type (if applicable).
	pub file_type: Option<&'a str>,
}

/// Registry of all hook definitions.
#[distributed_slice]
pub static HOOKS: [HookDef];

/// Emit an event to all registered hooks.
///
/// Hooks are executed in priority order (lower priority runs first).
/// Sync hooks complete immediately; async hooks are awaited in sequence.
///
/// Returns [`HookResult::Cancel`] if any hook cancels, otherwise [`HookResult::Continue`].
pub async fn emit(ctx: &HookContext<'_>) -> HookResult {
	let event = ctx.event();
	let mut matching: Vec<_> = HOOKS.iter().filter(|h| h.event == event).collect();
	matching.sort_by_key(|h| h.priority);

	for hook in matching {
		if hook.mutability != HookMutability::Immutable {
			continue;
		}
		let handler = match hook.handler {
			HookHandler::Immutable(handler) => handler,
			HookHandler::Mutable(_) => continue,
		};
		let result = match handler(ctx) {
			HookAction::Done(result) => result,
			HookAction::Async(fut) => fut.await,
		};
		if result == HookResult::Cancel {
			return HookResult::Cancel;
		}
	}
	HookResult::Continue
}

/// Emit an event synchronously, ignoring any async hooks.
///
/// This is useful in contexts where async is not available. Async hooks
/// will log a warning and be skipped.
pub fn emit_sync(ctx: &HookContext<'_>) -> HookResult {
	let event = ctx.event();
	let mut matching: Vec<_> = HOOKS.iter().filter(|h| h.event == event).collect();
	matching.sort_by_key(|h| h.priority);

	for hook in matching {
		if hook.mutability != HookMutability::Immutable {
			continue;
		}
		let handler = match hook.handler {
			HookHandler::Immutable(handler) => handler,
			HookHandler::Mutable(_) => continue,
		};
		match handler(ctx) {
			HookAction::Done(result) => {
				if result == HookResult::Cancel {
					return HookResult::Cancel;
				}
			}
			HookAction::Async(_) => {
				warn!(
					hook = hook.name,
					"Hook returned async action but emit_sync was called; skipping"
				);
			}
		}
	}
	HookResult::Continue
}

/// Emit a mutable event to all registered mutable hooks.
///
/// Returns [`HookResult::Cancel`] if any hook cancels, otherwise [`HookResult::Continue`].
pub async fn emit_mutable(ctx: &mut MutableHookContext<'_>) -> HookResult {
	let event = ctx.event;
	let mut matching: Vec<_> = HOOKS.iter().filter(|h| h.event == event).collect();
	matching.sort_by_key(|h| h.priority);

	for hook in matching {
		if hook.mutability != HookMutability::Mutable {
			continue;
		}
		let handler = match hook.handler {
			HookHandler::Mutable(handler) => handler,
			HookHandler::Immutable(_) => continue,
		};
		let result = match handler(ctx) {
			HookAction::Done(result) => result,
			HookAction::Async(fut) => fut.await,
		};
		if result == HookResult::Cancel {
			return HookResult::Cancel;
		}
	}
	HookResult::Continue
}

/// Find all hooks registered for a specific event.
pub fn find_hooks(event: HookEvent) -> impl Iterator<Item = &'static HookDef> {
	HOOKS.iter().filter(move |h| h.event == event)
}

/// List all registered hooks.
pub fn all_hooks() -> &'static [HookDef] {
	&HOOKS
}

/// Trait for scheduling async hook futures.
///
/// This allows sync emission to queue async hooks without coupling `evildoer-manifest`
/// to any specific runtime. The caller provides an implementor that stores futures
/// for later execution.
pub trait HookScheduler {
	/// Queue an async hook future for later execution.
	fn schedule(&mut self, fut: BoxFuture);
}

/// Emit an event synchronously, scheduling async hooks for later execution.
///
/// Sync hooks run immediately and can cancel the operation. Async hooks are
/// queued via the provided scheduler and will run later (they cannot cancel
/// since the operation has already proceeded).
///
/// Returns [`HookResult::Cancel`] if any sync hook cancels, otherwise [`HookResult::Continue`].
pub fn emit_sync_with<S: HookScheduler>(ctx: &HookContext<'_>, scheduler: &mut S) -> HookResult {
	let event = ctx.event();
	let mut matching: Vec<_> = HOOKS.iter().filter(|h| h.event == event).collect();
	matching.sort_by_key(|h| h.priority);

	for hook in matching {
		if hook.mutability != HookMutability::Immutable {
			continue;
		}
		let handler = match hook.handler {
			HookHandler::Immutable(handler) => handler,
			HookHandler::Mutable(_) => continue,
		};
		match handler(ctx) {
			HookAction::Done(result) => {
				if result == HookResult::Cancel {
					return HookResult::Cancel;
				}
			}
			HookAction::Async(fut) => {
				scheduler.schedule(fut);
			}
		}
	}
	HookResult::Continue
}
