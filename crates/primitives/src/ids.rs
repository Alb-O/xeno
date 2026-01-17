//! Identifier types for editor entities.

/// Unique identifier for a view (buffer instance).
///
/// Each view represents an independent editing context with its own cursor,
/// selection, and scroll position. Multiple views can share the same underlying
/// document (for split views), but each has a unique `ViewId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewId(pub u64);

impl ViewId {
	/// Identifier for the default scratch buffer.
	pub const SCRATCH: ViewId = ViewId(0);

	/// Creates a new view ID for a text buffer.
	///
	/// This is a convenience constructor that matches the hook API.
	pub const fn text(id: u64) -> Self {
		Self(id)
	}
}
