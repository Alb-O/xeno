//! Hook type definitions: HookDef, HookAction, HookResult.

use std::future::Future;
use std::pin::Pin;

pub use evildoer_registry_core::{RegistryMetadata, RegistrySource, impl_registry_metadata};

use super::HookEvent;
use super::context::{HookContext, MutableHookContext};

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
	/// Hook only reads state.
	Immutable,
	/// Hook may modify state.
	Mutable,
}

/// Handler function for a hook.
#[derive(Clone, Copy)]
pub enum HookHandler {
	/// Handler that receives immutable context.
	Immutable(fn(&HookContext) -> HookAction),
	/// Handler that receives mutable context.
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

impl_registry_metadata!(HookDef);
