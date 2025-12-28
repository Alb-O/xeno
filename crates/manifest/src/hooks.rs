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

use std::path::Path;
use std::pin::Pin;

use futures::future::Future;
use linkme::distributed_slice;
use ropey::RopeSlice;

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

/// Context passed to hook handlers.
///
/// Contains event-specific data that hooks can read but not modify.
/// For hooks that need to modify state, use [`MutableHookContext`].
pub enum HookContext<'a> {
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
	BufferClose { path: &'a Path },
	/// Buffer content changed.
	BufferChange { path: &'a Path, text: RopeSlice<'a> },
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

impl<'a> HookContext<'a> {
	pub fn event(&self) -> HookEvent {
		match self {
			HookContext::EditorStart => HookEvent::EditorStart,
			HookContext::EditorQuit => HookEvent::EditorQuit,
			HookContext::EditorTick => HookEvent::EditorTick,
			HookContext::BufferOpen { .. } => HookEvent::BufferOpen,
			HookContext::BufferWritePre { .. } => HookEvent::BufferWritePre,
			HookContext::BufferWrite { .. } => HookEvent::BufferWrite,
			HookContext::BufferClose { .. } => HookEvent::BufferClose,
			HookContext::BufferChange { .. } => HookEvent::BufferChange,
			HookContext::ModeChange { .. } => HookEvent::ModeChange,
			HookContext::CursorMove { .. } => HookEvent::CursorMove,
			HookContext::SelectionChange { .. } => HookEvent::SelectionChange,
			HookContext::WindowResize { .. } => HookEvent::WindowResize,
			HookContext::FocusGained => HookEvent::FocusGained,
			HookContext::FocusLost => HookEvent::FocusLost,
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
	pub priority: i32,
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

/// A mutable hook that can modify editor state.
#[derive(Clone, Copy)]
pub struct MutableHookDef {
	/// Hook name for debugging/logging.
	pub name: &'static str,
	/// The event this hook responds to.
	pub event: HookEvent,
	/// Short description.
	pub description: &'static str,
	/// Priority (lower runs first, default 100).
	pub priority: i32,
	/// The hook handler function.
	pub handler: fn(&mut MutableHookContext) -> HookAction,
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
				log::warn!(
					"Hook '{}' returned async action but emit_sync was called; skipping",
					hook.name
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
