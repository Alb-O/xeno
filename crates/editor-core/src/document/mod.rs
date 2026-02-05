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

use super::undo_store::{DocumentSnapshot, UndoBackend};

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

	/// Undo backend (snapshot or transaction-based).
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

	/// Creates a new document with transaction-based undo.
	///
	/// Transaction-based undo stores edit deltas instead of full rope snapshots,
	/// which can be more efficient for large documents with small edits.
	pub fn with_transaction_undo(content: String, path: Option<PathBuf>) -> Self {
		let mut doc = Self::new(content, path);
		doc.undo_backend = UndoBackend::transaction();
		doc
	}

	/// Switches to transaction-based undo backend.
	///
	/// Note: This clears any existing undo history.
	pub fn use_transaction_undo(&mut self) {
		self.undo_backend = UndoBackend::transaction();
	}

	/// Switches to snapshot-based undo backend.
	///
	/// Note: This clears any existing undo history.
	pub fn use_snapshot_undo(&mut self) {
		self.undo_backend = UndoBackend::snapshot();
	}

	/// Initializes syntax highlighting for this document based on file path.
	///
	/// This only sets the `language_id` and `file_type` fields. Actual syntax
	/// loading is deferred to the application layer for async background parsing.
	pub fn init_syntax(&mut self, language_loader: &LanguageLoader) {
		if let Some(ref p) = self.path
			&& let Some(lang_id) = language_loader.language_for_path(p)
		{
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.language_id = Some(lang_id);
			// Syntax loading deferred to SyntaxManager - do NOT call Syntax::new here
		}
	}

	/// Initializes syntax highlighting for this document by language name.
	///
	/// This only sets the `language_id` and `file_type` fields. Actual syntax
	/// loading is deferred to the application layer for async background parsing.
	pub fn init_syntax_for_language(&mut self, name: &str, language_loader: &LanguageLoader) {
		if let Some(lang_id) = language_loader.language_for_name(name) {
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.language_id = Some(lang_id);
			// Syntax loading deferred to SyntaxManager - do NOT call Syntax::new here
		}
	}

	/// Records the current document state as an undo boundary.
	///
	/// Creates a new undo step and ends any active insert grouping session.
	/// Call this before discrete edit operations (delete, change, paste, etc.).
	///
	/// View state (cursor, selection, scroll) is captured separately by the
	/// application layer.
	pub fn record_undo_boundary(&mut self) {
		self.insert_undo_active = false;
		let before = DocumentSnapshot {
			rope: self.content.clone(),
			version: self.version,
		};
		let empty_tx = xeno_primitives::Transaction::new(self.content.slice(..));
		self.undo_backend.record_commit(&empty_tx, &before);
	}

	/// Undoes the last document change.
	///
	/// Restores document content from the undo stack.
	/// View state restoration is handled by the application layer.
	///
	/// Returns the applied inverse transaction on success, or `None` if
	/// nothing to undo.
	///
	pub fn undo(&mut self, language_loader: &LanguageLoader) -> Option<Transaction> {
		self.insert_undo_active = false;

		if !self.undo_backend.can_undo() {
			return None;
		}

		let tx = self.undo_backend.undo(
			&mut self.content,
			&mut self.version,
			language_loader,
			|_, _| {},
		)?;
		Some(tx)
	}

	/// Redoes the last undone document change.
	///
	/// Restores document content from the redo stack.
	/// View state restoration is handled by the application layer.
	///
	/// Returns the applied transaction on success, or `None` if nothing to redo.
	///
	pub fn redo(&mut self, language_loader: &LanguageLoader) -> Option<Transaction> {
		self.insert_undo_active = false;

		if !self.undo_backend.can_redo() {
			return None;
		}

		let tx = self.undo_backend.redo(
			&mut self.content,
			&mut self.version,
			language_loader,
			|_, _| {},
		)?;
		Some(tx)
	}

	/// Applies an edit through the authoritative edit gate.
	///
	/// This is the single entry point for document modifications, ensuring:
	/// - Readonly checks
	/// - Undo recording (based on policy)
	/// - Transaction application
	/// - Version/modified flag updates
	/// - Redo stack clearing
	/// - Syntax policy outcome (no parsing in core)
	///
	/// View state (cursor, selection, scroll) capture happens at the editor level
	/// before calling this method. Document history is purely about document state.
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
	/// been validated at the buffer level. External code should use [`commit`].
	///
	/// Handles undo recording based on [`UndoPolicy`]: `NoUndo` skips recording,
	/// `Record`/`Boundary` creates a new snapshot, and `MergeWithCurrentGroup`
	/// only creates a snapshot if not already in an insert grouping session.
	///
	/// Syntax outcomes are policy-driven:
	/// - [`SyntaxPolicy::None`]: no syntax action
	/// - Other policies: report [`SyntaxOutcome::MarkedDirty`] so callers can
	///   schedule background parsing in the application layer.
	///
	/// [`Buffer`]: super::Buffer
	/// [`commit`]: Self::commit
	#[doc(hidden)]
	pub fn commit_unchecked(
		&mut self,
		commit: EditCommit,
		_language_loader: &LanguageLoader,
	) -> CommitResult {
		let version_before = self.version;
		let changed_ranges = collect_changed_ranges(&commit.tx);

		let should_record = match commit.undo {
			UndoPolicy::NoUndo => false,
			UndoPolicy::Record | UndoPolicy::Boundary => {
				self.insert_undo_active = false;
				true
			}
			UndoPolicy::MergeWithCurrentGroup => {
				if !self.insert_undo_active {
					self.insert_undo_active = true;
					true
				} else {
					false
				}
			}
		};

		let before = if should_record {
			Some(DocumentSnapshot {
				rope: self.content.clone(),
				version: self.version,
			})
		} else {
			None
		};

		commit.tx.apply(&mut self.content);
		self.modified = true;
		self.version = self.version.wrapping_add(1);

		let undo_recorded = if let Some(before) = before {
			self.undo_backend.record_commit(&commit.tx, &before);
			true
		} else {
			false
		};

		let syntax_outcome = match commit.syntax {
			SyntaxPolicy::None => SyntaxOutcome::Unchanged,
			SyntaxPolicy::MarkDirty
			| SyntaxPolicy::IncrementalOrDirty
			| SyntaxPolicy::FullReparseNow => SyntaxOutcome::MarkedDirty,
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
	/// **Warning:** This is a low-level escape hatch that bypasses undo recording
	/// and syntax updates. Prefer using `commit()` for normal edits, or
	/// `reset_content()` for wholesale content replacement in ephemeral buffers.
	#[allow(dead_code, reason = "escape hatch retained for internal migration")]
	pub(crate) fn content_mut(&mut self) -> &mut Rope {
		&mut self.content
	}

	/// Replaces the document content wholesale, clearing undo history.
	///
	/// This is intended for ephemeral buffers (info popups, prompts) where:
	/// - The entire content is replaced, not edited incrementally
	/// - Undo history doesn't make sense for the use case
	/// - The buffer will typically be set to readonly afterwards
	///
	/// For normal editing operations, use `commit()` instead.
	///
	/// Note: The document version is bumped to invalidate caches keyed by
	/// `doc.version()`.
	pub fn reset_content(&mut self, content: impl Into<Rope>) {
		self.content = content.into();
		self.insert_undo_active = false;
		self.undo_backend = UndoBackend::default();
		self.modified = false;
		self.version = self.version.wrapping_add(1);
	}

	/// Replaces the document content from a cross-process sync snapshot.
	///
	/// Unlike [`reset_content`], this preserves file document semantics:
	/// - Version increments monotonically (not reset to 0).
	/// - Modified flag is set to `true` (content differs from disk).
	/// - Undo history is cleared (undo across ownership boundaries is unsound).
	///   Use this for shared state follower join and full resync, where the remote
	///   content replaces local content but the document still represents a file.
	pub fn install_sync_snapshot(&mut self, content: impl Into<Rope>) {
		self.content = content.into();
		self.insert_undo_active = false;
		self.undo_backend = UndoBackend::default();
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
	///
	/// Incremented on every transaction. Used for LSP sync and cache invalidation.
	pub fn version(&self) -> u64 {
		self.version
	}

	/// Returns whether an insert undo group is currently active.
	///
	/// When `true`, subsequent edits with [`UndoPolicy::MergeWithCurrentGroup`]
	/// will be grouped with the current undo step. Used by the application layer
	/// to determine whether to push a new view-level undo group.
	pub fn insert_undo_active(&self) -> bool {
		self.insert_undo_active
	}

	/// Increments the document version. Called internally during transaction application.
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
