//! Machine-checkable invariant proofs for the buffer subsystem.
//!
//! Each invariant is expressed as a `pub(crate) fn test_*()` that is both
//! a runnable test and an intra-doc link target for the anchor module-level docs.

use xeno_primitives::DocumentId;

use super::LockGuard;
use crate::buffer::{Buffer, ViewId};

/// Must panic on re-entrant locking of the same document on one thread.
///
/// * Enforced in: `crate::buffer::LockGuard::new`
/// * Failure symptom: Self-deadlock in nested document access paths.
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

/// Must clamp cursor and selection state to current document bounds.
///
/// * Enforced in: `crate::buffer::Buffer::ensure_valid_selection`
/// * Failure symptom: Out-of-bounds cursor/selection causes rendering and edit panics.
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

/// Must keep document versions monotonically increasing across non-identity edits.
///
/// * Enforced in: `crate::buffer::editing::apply`, `crate::core::document::Document::commit_unchecked`
/// * Failure symptom: Stale-version logic accepts older edits or skips incremental updates.
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
