//! Document - the shared, file-backed content of a buffer.
//!
//! A [`Document`] represents the actual file content, separate from any view state.
//! Multiple buffers can reference the same document, enabling split views of
//! the same file with shared undo history.
//!
//! # History Separation
//!
//! Document history is purely about document state (text content). View state
//! (cursor, selection, scroll position) is managed by the application layer.

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use xeno_primitives::transaction::Operation;
use xeno_primitives::{
	CommitResult, EditCommit, EditError, Range, ReadOnlyReason, ReadOnlyScope, Rope, SyntaxOutcome,
	SyntaxPolicy, Transaction, UndoPolicy,
};
use xeno_runtime_language::LanguageLoader;

use super::undo_store::UndoBackend;

/// Counter for generating unique document IDs.
static NEXT_DOCUMENT_ID: AtomicU64 = AtomicU64::new(1);

/// Unique identifier for a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(pub u64);

impl DocumentId {
	/// Generates a new unique document ID.
	pub fn next() -> Self {
		Self(NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed))
	}

	/// The scratch document ID (for unsaved buffers).
	pub const SCRATCH: DocumentId = DocumentId(0);
}

/// A document - the shared, file-backed content.
///
/// Documents hold the actual text content and metadata that's shared across
/// all views of the same file. When you split a buffer, both views reference
/// the same document, so edits in one view appear in the other.
///
/// # Undo/Redo
///
/// Document-level undo history stores only document state (text content and
/// version). View state (cursor, selection, scroll) is managed by the
/// application layer. This clean separation means:
///
/// - Document undo affects all views of the same document
/// - Each view's cursor/selection is restored from the app-level snapshot
/// - Buffers can be created/destroyed without corrupting undo history
///
/// # Field Access
///
/// Core fields are private to enforce invariants. Use the provided getter methods
/// for read access, and the controlled mutation methods (like `commit()` when
/// available) for modifications.
pub struct Document {
	/// Unique identifier for this document.
	pub id: DocumentId,

	/// The text content.
	content: Rope,

	/// Associated file path (None for scratch documents).
	pub path: Option<PathBuf>,

	/// Whether the document has unsaved changes.
	modified: bool,

	/// Whether the document is read-only (prevents all text modifications).
	readonly: bool,

	/// Undo backend (standardized on transaction-based).
	///
	/// Manages document-level undo history. View state (cursor, selection,
	/// scroll) is handled by the application layer.
	undo_backend: UndoBackend,

	/// Detected file type (e.g., "rust", "python").
	pub file_type: Option<String>,

	/// Language ID for syntax highlighting (set by init_syntax).
	/// Actual syntax parsing is deferred to the application layer.
	language_id: Option<xeno_runtime_language::LanguageId>,

	/// Flag for grouping insert-mode edits into a single undo.
	insert_undo_active: bool,

	/// Document version, incremented on every transaction.
	///
	/// Used for LSP synchronization and cache invalidation.
	version: u64,
}

impl Document {
	/// Creates a new document with the given content and optional file path.
	pub fn new(content: String, path: Option<PathBuf>) -> Self {
		Self {
			id: DocumentId::next(),
			content: Rope::from(content.as_str()),
			path,
			modified: false,
			readonly: false,
			undo_backend: UndoBackend::default(),
			file_type: None,
			language_id: None,
			insert_undo_active: false,
			version: 0,
		}
	}

	/// Creates a new scratch document (no file path).
	pub fn scratch() -> Self {
		Self::new(String::new(), None)
	}

	/// Initializes syntax highlighting for this document based on file path.
	///
	/// This only sets metadata. Actual syntax loading is deferred to the
	/// application layer for async background parsing.
	pub fn init_syntax(&mut self, language_loader: &LanguageLoader) {
		if let Some(ref p) = self.path
			&& let Some(lang_id) = language_loader.language_for_path(p)
		{
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.language_id = Some(lang_id);
		}
	}

	/// Initializes syntax highlighting for this document by language name.
	pub fn init_syntax_for_language(&mut self, name: &str, language_loader: &LanguageLoader) {
		if let Some(lang_id) = language_loader.language_for_name(name) {
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.language_id = Some(lang_id);
		}
	}

	/// Records the current document state as an undo boundary.
	///
	/// Ends any active insert grouping session. Subsequent edits will
	/// start a new undo step.
	pub fn record_undo_boundary(&mut self) {
		self.insert_undo_active = false;
	}

	/// Undoes the last document change.
	///
	/// Restores document content from the undo stack. View state restoration
	/// is handled by the application layer.
	///
	/// # Returns
	///
	/// Returns the applied inverse transactions on success, or `None` if
	/// nothing to undo.
	pub fn undo(&mut self) -> Option<Vec<Transaction>> {
		self.insert_undo_active = false;
		self.undo_backend.undo(&mut self.content, &mut self.version)
	}

	/// Redoes the last undone document change.
	///
	/// Restores document content from the redo stack. View state restoration
	/// is handled by the application layer.
	///
	/// # Returns
	///
	/// Returns the applied transactions on success, or `None` if nothing to redo.
	pub fn redo(&mut self) -> Option<Vec<Transaction>> {
		self.insert_undo_active = false;
		self.undo_backend.redo(&mut self.content, &mut self.version)
	}

	/// Applies an edit through the authoritative edit gate.
	///
	/// This is the single entry point for document modifications, ensuring:
	/// - Readonly checks
	/// - Undo recording (based on policy)
	/// - Transaction application
	/// - Version/modified flag updates
	/// - Redo stack clearing
	/// - Syntax policy outcome
	///
	/// View state capture happens at the editor level before calling this method.
	///
	/// # Errors
	///
	/// Returns `EditError::ReadOnly` if the document is readonly.
	pub fn commit(
		&mut self,
		commit: EditCommit,
		language_loader: &LanguageLoader,
	) -> Result<CommitResult, EditError> {
		self.ensure_writable()?;
		Ok(self.commit_unchecked(commit, language_loader))
	}

	/// Applies an edit bypassing the readonly check.
	///
	/// For internal use by [`Buffer`] when the readonly override has already
	/// been validated. Handles undo recording and syntax policy outcomes.
	///
	/// [`Buffer`]: super::Buffer
	#[doc(hidden)]
	pub fn commit_unchecked(
		&mut self,
		commit: EditCommit,
		_language_loader: &LanguageLoader,
	) -> CommitResult {
		let version_before = self.version;
		let changed_ranges = collect_changed_ranges(&commit.tx);

		let (should_record, is_merge) = match commit.undo {
			UndoPolicy::NoUndo => (false, false),
			UndoPolicy::Record | UndoPolicy::Boundary => {
				self.insert_undo_active = false;
				(true, false)
			}
			UndoPolicy::MergeWithCurrentGroup => {
				let merge = self.insert_undo_active;
				if !merge {
					self.insert_undo_active = true;
				}
				(true, merge)
			}
		};

		let content_before = if should_record {
			Some(self.content.clone())
		} else {
			None
		};

		commit.tx.apply(&mut self.content);
		self.modified = true;
		self.version = self.version.wrapping_add(1);

		// Any new edit invalidates the redo stack.
		self.undo_backend.clear_redo();

		let undo_recorded = if let Some(before) = content_before {
			self.undo_backend
				.record_commit(&commit.tx, &before, is_merge);
			!is_merge
		} else {
			false
		};

		let syntax_outcome = match commit.syntax {
			SyntaxPolicy::None => SyntaxOutcome::Unchanged,
			_ => SyntaxOutcome::MarkedDirty,
		};

		CommitResult {
			applied: true,
			version_before,
			version_after: self.version,
			selection_after: commit.selection_after,
			undo_recorded,
			insert_group_active_after: self.insert_undo_active,
			changed_ranges: changed_ranges.into(),
			syntax_outcome,
		}
	}

	/// Checks if the document is writable, returning an error if readonly.
	fn ensure_writable(&self) -> Result<(), EditError> {
		if self.readonly {
			return Err(EditError::ReadOnly {
				scope: ReadOnlyScope::Document,
				reason: ReadOnlyReason::FlaggedReadOnly,
			});
		}
		Ok(())
	}

	/// Returns a reference to the document's text content.
	pub fn content(&self) -> &Rope {
		&self.content
	}

	/// Returns a mutable reference to the document's text content.
	///
	/// **Warning:** This is a low-level escape hatch that bypasses history
	/// and syntax updates. Prefer `commit()`.
	#[allow(dead_code, reason = "escape hatch retained for internal migration")]
	pub(crate) fn content_mut(&mut self) -> &mut Rope {
		&mut self.content
	}

	/// Replaces the document content wholesale, clearing undo history.
	///
	/// Intended for ephemeral buffers where incremental editing is not used.
	pub fn reset_content(&mut self, content: impl Into<Rope>) {
		self.content = content.into();
		self.insert_undo_active = false;
		self.undo_backend = UndoBackend::new();
		self.modified = false;
		self.version = self.version.wrapping_add(1);
	}

	/// Replaces the document content from a sync snapshot.
	///
	/// preserved version monotonicity but clears history.
	pub fn install_sync_snapshot(&mut self, content: impl Into<Rope>) {
		self.content = content.into();
		self.insert_undo_active = false;
		self.undo_backend = UndoBackend::new();
		self.modified = true;
		self.version = self.version.wrapping_add(1);
	}

	/// Returns whether the document has unsaved changes.
	pub fn is_modified(&self) -> bool {
		self.modified
	}

	/// Sets the modified flag.
	pub fn set_modified(&mut self, modified: bool) {
		self.modified = modified;
	}

	/// Returns whether the document is read-only.
	pub fn is_readonly(&self) -> bool {
		self.readonly
	}

	/// Sets the read-only flag.
	pub fn set_readonly(&mut self, readonly: bool) {
		self.readonly = readonly;
	}

	/// Returns the document version.
	pub fn version(&self) -> u64 {
		self.version
	}

	/// Returns whether an insert undo group is currently active.
	pub fn insert_undo_active(&self) -> bool {
		self.insert_undo_active
	}

	/// Increments the document version.
	#[doc(hidden)]
	pub fn increment_version(&mut self) {
		self.version = self.version.wrapping_add(1);
	}

	/// Returns the number of items in the undo stack.
	pub fn undo_len(&self) -> usize {
		self.undo_backend.undo_len()
	}

	/// Returns the number of items in the redo stack.
	pub fn redo_len(&self) -> usize {
		self.undo_backend.redo_len()
	}

	/// Returns whether undo is available.
	pub fn can_undo(&self) -> bool {
		self.undo_backend.can_undo()
	}

	/// Returns whether redo is available.
	pub fn can_redo(&self) -> bool {
		self.undo_backend.can_redo()
	}

	/// Returns the language ID for this document.
	pub fn language_id(&self) -> Option<xeno_runtime_language::LanguageId> {
		self.language_id
	}

	/// Resets the insert undo grouping flag.
	#[doc(hidden)]
	pub fn reset_insert_undo(&mut self) {
		self.insert_undo_active = false;
	}
}

/// Collects ranges affected by a transaction in pre-edit coordinates.
pub(crate) fn collect_changed_ranges(tx: &xeno_primitives::Transaction) -> Vec<Range> {
	let mut ranges = Vec::new();
	let mut pos = 0;

	for op in tx.operations() {
		match op {
			Operation::Retain(n) => {
				pos += n;
			}
			Operation::Delete(n) => {
				push_range(&mut ranges, pos, pos + n);
				pos += n;
			}
			Operation::Insert(_) => {
				push_range(&mut ranges, pos, pos);
			}
		}
	}

	ranges
}

fn push_range(ranges: &mut Vec<Range>, start: usize, end: usize) {
	let start = start.min(end);
	let end = start.max(end);

	if let Some(last) = ranges.last_mut() {
		let last_start = last.from();
		let last_end = last.to();
		if start <= last_end {
			let merged_start = last_start.min(start);
			let merged_end = last_end.max(end);
			*last = Range::new(merged_start, merged_end);
			return;
		}
	}

	ranges.push(Range::new(start, end));
}
