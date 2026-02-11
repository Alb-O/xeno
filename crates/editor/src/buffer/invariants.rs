//! Machine-checkable invariant proofs for the buffer subsystem.
//!
//! Each invariant is expressed as a `pub(crate) fn test_*()` that is both
//! a runnable test and an intra-doc link target for the anchor module-level docs.

use super::LockGuard;
use crate::buffer::{Buffer, ViewId};
use crate::core::document::DocumentId;

/// Invariant: Re-entrant locking of the same document on a single thread MUST panic.
#[cfg_attr(test, test)]
pub(crate) fn test_reentrant_lock_panic() {
	let id = DocumentId(0);

	// First lock should succeed.
	let _guard = LockGuard::new(id);

	// Second lock on the same document on the same thread MUST panic.
	let result = std::panic::catch_unwind(|| {
		let _inner = LockGuard::new(id);
	});
	assert!(result.is_err(), "re-entrant lock did not panic");

	// After the panic, the outer guard is still held.
	// Dropping it should clean up without issue.
	drop(_guard);
}

/// Invariant: View state (cursor/selection) MUST be clamped within document bounds.
#[cfg_attr(test, test)]
pub(crate) fn test_selection_clamping() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);

	// Buffer starts empty (0 chars).
	// Force cursor beyond bounds.
	buffer.cursor = 100;

	buffer.ensure_valid_selection();

	let max_char = buffer.with_doc(|doc| doc.content().len_chars());
	assert!(buffer.cursor <= max_char, "cursor should be clamped to document bounds");
}

/// Invariant: Document versions MUST be monotonically increasing across edits.
#[cfg_attr(test, test)]
pub(crate) fn test_version_monotonicity() {
	use crate::buffer::ApplyPolicy;

	let mut buffer = Buffer::scratch(ViewId::SCRATCH);

	let v0 = buffer.with_doc(|doc| doc.version());

	// Apply a real edit.
	let (tx, _sel) = buffer.prepare_insert("hello");
	let result = buffer.apply(&tx, ApplyPolicy::INTERNAL);
	let v1 = result.version_after;
	assert!(v1 > v0, "version must increase after non-identity edit");

	// Apply another edit.
	let (tx2, _sel2) = buffer.prepare_insert(" world");
	let result2 = buffer.apply(&tx2, ApplyPolicy::INTERNAL);
	let v2 = result2.version_after;
	assert!(v2 > v1, "version must continue increasing");
}
