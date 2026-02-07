//! Sole capability provider for registry actions.
//!
//! # Purpose
//!
//! This module provides the [`EditorCaps`] delegator, which is the sole implementor
//! of registry capability traits in the `xeno-editor` crate.
//!
//! # Invariants
//!
//! - The Delegator Rule: The [`Editor`] struct must not implement any `xeno_registry::*Access`
//!   traits. All registry-facing capabilities must be implemented on [`EditorCaps`].
//!   - Enforced in: [`crate::capabilities::provider::EditorCaps`] (via delegator pattern)
//!   - Tested by: [`crate::capabilities::invariants::test_delegator_rule`]
//!   - Failure symptom: Circular dependencies or accidental leakage of engine-specific methods into the action registry.
//!
//! - Mutation Side-Effect Invariant: Capability methods on `EditorCaps` that mutate editor state
//!   must enqueue corresponding events into the `EffectSink`.
//!   - Enforced in: [`crate::capabilities::provider::EditorCaps`] (via domain-specific implementations)
//!   - Tested by: [`crate::capabilities::invariants::test_mutation_side_effect_invariant`]
//!   - Failure symptom: UI layers (overlays, status bars) failing to update after an action executes.
//!
//! # Enforcement (The Delegator Rule)
//!
//! The following "tripwire" ensures that [`Editor`] does not implement [`CursorAccess`] directly.
//! If this block fails to compile, it means the Delegator Rule has been regressed.
//!
//! ```compile_fail
//! use xeno_registry::CursorAccess;
//! use crate::impls::Editor;
//! fn _assert_is_not_cursor_access(e: &mut Editor) {
//!     let _x: &mut dyn CursorAccess = e;
//! }
//! ```
//!
//! [`Editor`]: crate::impls::Editor
//! [`CursorAccess`]: xeno_registry::CursorAccess

use crate::impls::Editor;

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
