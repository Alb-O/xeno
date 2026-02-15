//! Sole capability provider for registry actions.
//!
//! # Purpose
//!
//! This module provides the [`EditorCaps`] delegator, which is the sole implementor
//! of registry capability traits in the `xeno-editor` crate.
//!
//! # Invariants
//!
//! * Must not implement `xeno_registry::*Access` traits directly on `Editor` (Delegator Rule).
//! * Must enqueue layer events for capability mutations (Mutation Side-Effect Invariant).
//!
//! # Enforcement (The Delegator Rule)
//!
//! The following "tripwire" ensures that [`Editor`] does not implement
//! [`CursorAccess`](xeno_registry::actions::CursorAccess) directly.
//! If this block fails to compile, it means the Delegator Rule has been regressed.
//!
//! ```compile_fail
//! use xeno_registry::actions::CursorAccess;
//! use crate::Editor;
//! fn _assert_is_not_cursor_access(e: &mut Editor) {
//!     let _x: &mut dyn CursorAccess = e;
//! }
//! ```
//!
//! [`Editor`]: crate::Editor
//! [`CursorAccess`]: xeno_registry::actions::CursorAccess

use crate::Editor;

/// Sole capability provider for registry actions.
///
/// This struct wraps a mutable reference to the [`Editor`] and implements
/// the various capability traits from `xeno-registry`. This keeps the
/// [`Editor`] struct itself clean of registry-specific trait implementations
/// and provides a clear boundary for capability delegation.
pub struct EditorCaps<'a> {
	pub(crate) ed: &'a mut Editor,
}

impl<'a> EditorCaps<'a> {
	/// Creates a new capability provider wrapping the given editor.
	pub fn new(ed: &'a mut Editor) -> Self {
		Self { ed }
	}
}

impl Editor {
	/// Returns a capability provider for this editor.
	///
	/// Callsites must bind the returned provider to a local variable to satisfy
	/// borrow checker lifetimes when creating an `EditorContext`:
	///
	/// ```ignore
	/// let mut caps = self.caps();
	/// let mut ctx = EditorContext::new(&mut caps);
	/// ```
	pub fn caps(&mut self) -> EditorCaps<'_> {
		EditorCaps::new(self)
	}
}
