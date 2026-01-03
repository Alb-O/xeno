//! Hook context types for passing event data to handlers.

use std::any::Any;
use std::path::Path;

use evildoer_base::Rope;
pub use evildoer_registry_panels::PanelId;

use super::{HookEvent, HookEventData, OwnedHookContext};

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

/// Mutable context passed to mutable hook handlers.
pub struct MutableHookContext<'a> {
	/// The event being processed.
	pub event: HookEvent,
	/// Mutable document content (if applicable).
	pub text: Option<&'a mut Rope>,
	/// File path (if applicable).
	pub path: Option<&'a Path>,
	/// File type (if applicable).
	pub file_type: Option<&'a str>,
}
