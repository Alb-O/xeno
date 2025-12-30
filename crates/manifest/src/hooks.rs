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
use ropey::RopeSlice;
use tracing::warn;

use crate::{Mode, RegistrySource};

/// Events that can trigger hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
	/// Editor is starting up (before first render).
	EditorStart,
	/// Editor is shutting down.
	EditorQuit,
	/// Periodic tick.
	EditorTick,
	/// A buffer was opened/created.
	BufferOpen,
	/// A buffer is about to be written to disk.
	BufferWritePre,
	/// A buffer was written to disk.
	BufferWrite,
	/// A buffer was closed.
	BufferClose,
	/// Buffer content changed.
	BufferChange,
	/// Mode changed (normal -> insert, etc).
	ModeChange,
	/// Cursor position changed.
	CursorMove,
	/// Selection changed.
	SelectionChange,
	/// Window was resized.
	WindowResize,
	/// Window gained focus.
	FocusGained,
	/// Window lost focus.
	FocusLost,
}

impl HookEvent {
	pub fn as_str(&self) -> &'static str {
		match self {
			HookEvent::EditorStart => "editor:start",
			HookEvent::EditorQuit => "editor:quit",
			HookEvent::EditorTick => "editor:tick",
			HookEvent::BufferOpen => "buffer:open",
			HookEvent::BufferWritePre => "buffer:write-pre",
			HookEvent::BufferWrite => "buffer:write",
			HookEvent::BufferClose => "buffer:close",
			HookEvent::BufferChange => "buffer:change",
			HookEvent::ModeChange => "mode:change",
			HookEvent::CursorMove => "cursor:move",
			HookEvent::SelectionChange => "selection:change",
			HookEvent::WindowResize => "window:resize",
			HookEvent::FocusGained => "focus:gained",
			HookEvent::FocusLost => "focus:lost",
		}
	}
}

/// Event-specific data for hooks.
///
/// Contains the payload for each hook event type.
pub enum HookEventData<'a> {
	/// Editor startup context.
	EditorStart,
	/// Editor quit context.
	EditorQuit,
	/// Editor tick context.
	EditorTick,
	/// Buffer was opened.
	BufferOpen {
		path: &'a Path,
		text: RopeSlice<'a>,
		file_type: Option<&'a str>,
	},
	/// Buffer is about to be written.
	BufferWritePre { path: &'a Path, text: RopeSlice<'a> },
	/// Buffer was written.
	BufferWrite { path: &'a Path },
	/// Buffer was closed.
	BufferClose {
		path: &'a Path,
		file_type: Option<&'a str>,
	},
	/// Buffer content changed.
	BufferChange {
		path: &'a Path,
		text: RopeSlice<'a>,
		/// Detected file type (e.g., "rust", "python").
		file_type: Option<&'a str>,
		/// Document version number (incremented on each transaction).
		version: u64,
	},
	/// Mode changed.
	ModeChange { old_mode: Mode, new_mode: Mode },
	/// Cursor moved.
	CursorMove { line: usize, col: usize },
	/// Selection changed.
	SelectionChange { anchor: usize, head: usize },
	/// Window resized.
	WindowResize { width: u16, height: u16 },
	/// Window focus gained.
	FocusGained,
	/// Window focus lost.
	FocusLost,
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

impl<'a> HookEventData<'a> {
	/// Returns the event type for this data.
	pub fn event(&self) -> HookEvent {
		match self {
			HookEventData::EditorStart => HookEvent::EditorStart,
			HookEventData::EditorQuit => HookEvent::EditorQuit,
			HookEventData::EditorTick => HookEvent::EditorTick,
			HookEventData::BufferOpen { .. } => HookEvent::BufferOpen,
			HookEventData::BufferWritePre { .. } => HookEvent::BufferWritePre,
			HookEventData::BufferWrite { .. } => HookEvent::BufferWrite,
			HookEventData::BufferClose { .. } => HookEvent::BufferClose,
			HookEventData::BufferChange { .. } => HookEvent::BufferChange,
			HookEventData::ModeChange { .. } => HookEvent::ModeChange,
			HookEventData::CursorMove { .. } => HookEvent::CursorMove,
			HookEventData::SelectionChange { .. } => HookEvent::SelectionChange,
			HookEventData::WindowResize { .. } => HookEvent::WindowResize,
			HookEventData::FocusGained => HookEvent::FocusGained,
			HookEventData::FocusLost => HookEvent::FocusLost,
		}
	}

	/// Creates an owned version of this event data for use in async hooks.
	///
	/// Copies all data so it can be moved into a future.
	pub fn to_owned(&self) -> OwnedHookContext {
		match self {
			HookEventData::EditorStart => OwnedHookContext::EditorStart,
			HookEventData::EditorQuit => OwnedHookContext::EditorQuit,
			HookEventData::EditorTick => OwnedHookContext::EditorTick,
			HookEventData::BufferOpen {
				path,
				text,
				file_type,
			} => OwnedHookContext::BufferOpen {
				path: path.to_path_buf(),
				text: text.to_string(),
				file_type: file_type.map(String::from),
			},
			HookEventData::BufferWritePre { path, text } => OwnedHookContext::BufferWritePre {
				path: path.to_path_buf(),
				text: text.to_string(),
			},
			HookEventData::BufferWrite { path } => OwnedHookContext::BufferWrite {
				path: path.to_path_buf(),
			},
			HookEventData::BufferClose { path, file_type } => OwnedHookContext::BufferClose {
				path: path.to_path_buf(),
				file_type: file_type.map(String::from),
			},
			HookEventData::BufferChange {
				path,
				text,
				file_type,
				version,
			} => OwnedHookContext::BufferChange {
				path: path.to_path_buf(),
				text: text.to_string(),
				file_type: file_type.map(String::from),
				version: *version,
			},
			HookEventData::ModeChange { old_mode, new_mode } => OwnedHookContext::ModeChange {
				old_mode: old_mode.clone(),
				new_mode: new_mode.clone(),
			},
			HookEventData::CursorMove { line, col } => OwnedHookContext::CursorMove {
				line: *line,
				col: *col,
			},
			HookEventData::SelectionChange { anchor, head } => OwnedHookContext::SelectionChange {
				anchor: *anchor,
				head: *head,
			},
			HookEventData::WindowResize { width, height } => OwnedHookContext::WindowResize {
				width: *width,
				height: *height,
			},
			HookEventData::FocusGained => OwnedHookContext::FocusGained,
			HookEventData::FocusLost => OwnedHookContext::FocusLost,
		}
	}
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

/// Owned version of [`HookContext`] for async hook handlers.
///
/// Unlike `HookContext` which borrows data, this owns all its data and can be
/// moved into async futures. Use [`HookContext::to_owned()`] to create one.
///
/// # Example
///
/// ```ignore
/// hook!(lsp_open, BufferOpen, 100, "Notify LSP", |ctx| {
///     let owned = ctx.to_owned();
///     HookAction::Async(Box::pin(async move {
///         if let OwnedHookContext::BufferOpen { path, text, file_type } = owned {
///             lsp.did_open(&path, &text, file_type.as_deref()).await;
///         }
///         HookResult::Continue
///     }))
/// });
/// ```
#[derive(Debug, Clone)]
pub enum OwnedHookContext {
	/// Editor startup context.
	EditorStart,
	/// Editor quit context.
	EditorQuit,
	/// Editor tick context.
	EditorTick,
	/// Buffer was opened.
	BufferOpen {
		path: std::path::PathBuf,
		text: String,
		file_type: Option<String>,
	},
	/// Buffer is about to be written.
	BufferWritePre {
		path: std::path::PathBuf,
		text: String,
	},
	/// Buffer was written.
	BufferWrite { path: std::path::PathBuf },
	/// Buffer was closed.
	BufferClose {
		path: std::path::PathBuf,
		file_type: Option<String>,
	},
	/// Buffer content changed.
	BufferChange {
		path: std::path::PathBuf,
		text: String,
		file_type: Option<String>,
		version: u64,
	},
	/// Mode changed.
	ModeChange { old_mode: Mode, new_mode: Mode },
	/// Cursor moved.
	CursorMove { line: usize, col: usize },
	/// Selection changed.
	SelectionChange { anchor: usize, head: usize },
	/// Window resized.
	WindowResize { width: u16, height: u16 },
	/// Window focus gained.
	FocusGained,
	/// Window focus lost.
	FocusLost,
}

impl OwnedHookContext {
	/// Returns the event type for this context.
	pub fn event(&self) -> HookEvent {
		match self {
			OwnedHookContext::EditorStart => HookEvent::EditorStart,
			OwnedHookContext::EditorQuit => HookEvent::EditorQuit,
			OwnedHookContext::EditorTick => HookEvent::EditorTick,
			OwnedHookContext::BufferOpen { .. } => HookEvent::BufferOpen,
			OwnedHookContext::BufferWritePre { .. } => HookEvent::BufferWritePre,
			OwnedHookContext::BufferWrite { .. } => HookEvent::BufferWrite,
			OwnedHookContext::BufferClose { .. } => HookEvent::BufferClose,
			OwnedHookContext::BufferChange { .. } => HookEvent::BufferChange,
			OwnedHookContext::ModeChange { .. } => HookEvent::ModeChange,
			OwnedHookContext::CursorMove { .. } => HookEvent::CursorMove,
			OwnedHookContext::SelectionChange { .. } => HookEvent::SelectionChange,
			OwnedHookContext::WindowResize { .. } => HookEvent::WindowResize,
			OwnedHookContext::FocusGained => HookEvent::FocusGained,
			OwnedHookContext::FocusLost => HookEvent::FocusLost,
		}
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
	/// The hook handler function.
	///
	/// Returns [`HookAction::Done`] for sync completion or [`HookAction::Async`]
	/// with a future for async work.
	pub handler: fn(&HookContext) -> HookAction,
	/// Origin of the hook.
	pub source: RegistrySource,
}

impl std::fmt::Debug for HookDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("HookDef")
			.field("name", &self.name)
			.field("event", &self.event)
			.field("priority", &self.priority)
			.field("description", &self.description)
			.finish()
	}
}

impl crate::RegistryMetadata for HookDef {
	fn id(&self) -> &'static str {
		self.id
	}

	fn name(&self) -> &'static str {
		self.name
	}

	fn priority(&self) -> i16 {
		self.priority
	}

	fn source(&self) -> RegistrySource {
		self.source
	}
}

/// A mutable hook that can modify editor state.
#[derive(Clone, Copy)]
pub struct MutableHookDef {
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
	/// The hook handler function.
	pub handler: fn(&mut MutableHookContext) -> HookAction,
	/// Origin of the hook.
	pub source: RegistrySource,
}

impl std::fmt::Debug for MutableHookDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MutableHookDef")
			.field("name", &self.name)
			.field("event", &self.event)
			.field("priority", &self.priority)
			.field("description", &self.description)
			.finish()
	}
}

impl crate::RegistryMetadata for MutableHookDef {
	fn id(&self) -> &'static str {
		self.id
	}

	fn name(&self) -> &'static str {
		self.name
	}

	fn priority(&self) -> i16 {
		self.priority
	}

	fn source(&self) -> RegistrySource {
		self.source
	}
}

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

/// Registry of mutable hook definitions.
#[distributed_slice]
pub static MUTABLE_HOOKS: [MutableHookDef];

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
		let result = match (hook.handler)(ctx) {
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
		match (hook.handler)(ctx) {
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
	let mut matching: Vec<_> = MUTABLE_HOOKS.iter().filter(|h| h.event == event).collect();
	matching.sort_by_key(|h| h.priority);

	for hook in matching {
		let result = match (hook.handler)(ctx) {
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
		match (hook.handler)(ctx) {
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
