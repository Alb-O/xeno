//! Hook context types for passing event data to handlers.

use std::path::Path;

use xeno_primitives::Rope;
pub use xeno_primitives::ViewId;

use super::{HookEvent, HookEventData, OwnedHookContext};

/// Identifier for a window in hook payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

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

/// Window kinds for window lifecycle events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
	Base,
	Floating,
}

/// Context passed to hook handlers.
///
/// Contains event-specific data.
/// For hooks that need to modify state, use [`MutableHookContext`].
pub struct HookContext<'a> {
	/// The event-specific data.
	pub data: HookEventData<'a>,
}

impl<'a> HookContext<'a> {
	/// Creates a new hook context with event data.
	pub fn new(data: HookEventData<'a>) -> Self {
		Self { data }
	}

	/// Returns the event type for this context.
	pub fn event(&self) -> HookEvent {
		self.data.event()
	}

	/// Creates an owned version of the event data for use in async hooks.
	pub fn to_owned(&self) -> OwnedHookContext {
		self.data.to_owned()
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
