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

/// A text document representing shared, file-backed content.
///
/// Documents hold the authoritative text content and metadata shared across all
/// views (buffers) of the same file. They enforce a strict separation between
/// content state and view state (cursors, selection, scroll position).
///
/// # Architecture
///
/// - Content is stored as a [`Rope`] for efficient editing of large files.
/// - Edits are applied via [`Transaction`] objects through the [`commit`] method.
/// - History is managed by an [`UndoBackend`] at the document level, ensuring
///   undoing an edit affects all views of that document.
pub struct Document {
	/// Unique identifier for this document.
	pub id: DocumentId,
	/// The text content.
	content: Rope,
	/// Associated file path. `None` for scratch documents.
	pub path: Option<PathBuf>,
	/// Whether the document has unsaved changes.
	modified: bool,
	/// Whether the document is read-only (prevents all modifications).
	readonly: bool,
	/// Transaction-based grouped undo history.
	undo_backend: UndoBackend,
	/// Detected file type (e.g., "rust").
	pub file_type: Option<String>,
	/// Language ID used for syntax highlighting.
	language_id: Option<xeno_runtime_language::LanguageId>,
	/// Monotonic document version, incremented on every transaction.
	version: u64,
}

/// Static snapshot of a document's core state at a specific version.
///
/// Snapshots are cheap to create (using `Rope` cloning) and are designed to be
/// used outside document locks for background processing like syntax parsing or
/// LSP synchronization.
#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
	/// Identity of the source document.
	pub id: DocumentId,
	/// Version of the document at the time the snapshot was taken.
	pub version: u64,
	/// Language identity.
	pub language_id: Option<xeno_runtime_language::LanguageId>,
	/// Snapshot of the text content.
	pub content: Rope,
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
			version: 0,
		}
	}

	/// Captures a static snapshot of the document's current state.
	pub fn snapshot(&self) -> DocumentSnapshot {
		DocumentSnapshot {
			id: self.id,
			version: self.version,
			language_id: self.language_id,
			content: self.content.clone(),
		}
	}

	/// Creates a new scratch document without an associated file path.
	pub fn scratch() -> Self {
		Self::new(String::new(), None)
	}

	/// Initializes syntax highlighting metadata based on the file path.
	pub fn init_syntax(&mut self, language_loader: &LanguageLoader) {
		if let Some(ref p) = self.path
			&& let Some(lang_id) = language_loader.language_for_path(p)
		{
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.language_id = Some(lang_id);
		}
	}

	/// Initializes syntax highlighting metadata by explicit language name.
	pub fn init_syntax_for_language(&mut self, name: &str, language_loader: &LanguageLoader) {
		if let Some(lang_id) = language_loader.language_for_name(name) {
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.language_id = Some(lang_id);
		}
	}

	/// Undoes the last document change.
	///
	/// # Returns
	///
	/// The applied inverse transactions if undo was performed.
	pub fn undo(&mut self) -> Option<Vec<Transaction>> {
		self.undo_backend.undo(&mut self.content, &mut self.version)
	}

	/// Redoes the last undone document change.
	///
	/// # Returns
	///
	/// The applied forward transactions if redo was performed.
	pub fn redo(&mut self) -> Option<Vec<Transaction>> {
		self.undo_backend.redo(&mut self.content, &mut self.version)
	}

	/// Applies an edit through the authoritative edit gate.
	///
	/// This is the primary entry point for modifying document text. It handles
	/// readonly checks, versioning, and history recording.
	///
	/// # Errors
	///
	/// Returns [`EditError::ReadOnly`] if the document is flagged as read-only.
	pub fn commit(
		&mut self,
		commit: EditCommit,
		merge: bool,
		language_loader: &LanguageLoader,
	) -> Result<CommitResult, EditError> {
		self.ensure_writable()?;
		Ok(self.commit_unchecked(commit, merge, language_loader))
	}

	/// Applies an edit bypassing the readonly check.
	///
	/// # Safety
	///
	/// Internal use only. Callers MUST ensure that any readonly overrides
	/// have been validated at the view layer.
	#[doc(hidden)]
	pub fn commit_unchecked(
		&mut self,
		commit: EditCommit,
		merge: bool,
		_language_loader: &LanguageLoader,
	) -> CommitResult {
		let version_before = self.version;
		let changed_ranges = collect_changed_ranges(&commit.tx);

		let should_record = !matches!(commit.undo, UndoPolicy::NoUndo);

		let content_before = if should_record {
			Some(self.content.clone())
		} else {
			None
		};

		commit.tx.apply(&mut self.content);
		self.modified = true;
		self.version = self.version.wrapping_add(1);
		self.undo_backend.clear_redo();

		let undo_recorded = if let Some(before) = content_before {
			self.undo_backend.record_commit(&commit.tx, &before, merge);
			!merge
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
			changed_ranges: changed_ranges.into(),
			syntax_outcome,
		}
	}

	fn ensure_writable(&self) -> Result<(), EditError> {
		if self.readonly {
			return Err(EditError::ReadOnly {
				scope: ReadOnlyScope::Document,
				reason: ReadOnlyReason::FlaggedReadOnly,
			});
		}
		Ok(())
	}

	/// Returns a reference to the text content.
	pub fn content(&self) -> &Rope {
		&self.content
	}

	/// Returns a mutable reference to the text content.
	///
	/// # Warning
	///
	/// Low-level escape hatch. Bypasses history and versioning. Prefer [`commit`].
	#[allow(dead_code, reason = "escape hatch retained for internal migration")]
	pub(crate) fn content_mut(&mut self) -> &mut Rope {
		&mut self.content
	}

	/// Replaces the entire document content, clearing history.
	///
	/// Intended for ephemeral buffers where incremental editing is not used.
	pub fn reset_content(&mut self, content: impl Into<Rope>) {
		self.content = content.into();
		self.undo_backend = UndoBackend::new();
		self.modified = false;
		self.version = self.version.wrapping_add(1);
	}

	/// Replaces the document content from a synchronization snapshot.
	///
	/// Preserves document version monotonicity but clears local undo history.
	pub fn install_sync_snapshot(&mut self, content: impl Into<Rope>) {
		self.content = content.into();
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
