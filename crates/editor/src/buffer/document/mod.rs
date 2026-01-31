//! Document - the shared, file-backed content of a buffer.
//!
//! A [`Document`] represents the actual file content, separate from any view state.
//! Multiple buffers can reference the same document, enabling split views of
//! the same file with shared undo history.
//!
//! # History Separation
//!
//! Document history is purely about document state (text content). View state
//! (cursor, selection, scroll position) is managed at the editor level via
//! [`EditorUndoGroup`] and [`ViewSnapshot`].
//!
//! [`EditorUndoGroup`]: crate::types::EditorUndoGroup
//! [`ViewSnapshot`]: crate::types::ViewSnapshot

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
use xeno_runtime_language::syntax::Syntax;

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
/// version). View state (cursor, selection, scroll) is managed separately at
/// the editor level via [`EditorUndoGroup`]. This clean separation means:
///
/// - Document undo affects all views of the same document
/// - Each view's cursor/selection is restored from the editor-level snapshot
/// - Buffers can be created/destroyed without corrupting undo history
///
/// [`EditorUndoGroup`]: crate::types::EditorUndoGroup
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
	/// scroll) is handled separately at the editor level via [`EditorUndoGroup`].
	///
	/// [`EditorUndoGroup`]: crate::types::EditorUndoGroup
	undo_backend: UndoBackend,

	/// Detected file type (e.g., "rust", "python").
	pub file_type: Option<String>,

	/// Language ID for syntax highlighting (set by init_syntax).
	/// Actual Syntax loading is deferred to SyntaxManager.
	language_id: Option<xeno_runtime_language::LanguageId>,

	/// Syntax highlighting state.
	syntax: Option<Syntax>,

	/// Whether the syntax tree needs reparsing.
	///
	/// Set to `true` by commits with `SyntaxPolicy::MarkDirty` or
	/// `SyntaxPolicy::IncrementalOrDirty`. Cleared by `reparse_syntax` or
	/// `ensure_syntax_clean`.
	syntax_dirty: bool,

	/// Flag for grouping insert-mode edits into a single undo.
	insert_undo_active: bool,

	/// Document version, incremented on every transaction.
	///
	/// Used for LSP synchronization and cache invalidation.
	version: u64,

	/// Syntax version, incremented when the syntax tree is updated.
	///
	/// Used as a cache key for highlight spans. Tracks when the syntax tree
	/// actually updates, separate from `version` which tracks content changes.
	pub syntax_version: u64,
}

impl Document {
	const MAX_SYNC_INCREMENTAL_BYTES: usize = 256 * 1024; // Tier S only

	/// Whether a synchronous incremental syntax update is feasible.
	///
	/// Requires a syntax tree, a clean base (not dirty), and content within
	/// the Tier S size limit.
	fn can_sync_incremental(&self) -> bool {
		self.syntax.is_some()
			&& !self.syntax_dirty
			&& self.content.len_bytes() <= Self::MAX_SYNC_INCREMENTAL_BYTES
	}

	/// Incremental syntax update for undo/redo operations.
	///
	/// The snapshot undo backend records `undo_tx = tx.invert(&before)` only
	/// for the first keystroke of a merged insert-mode group. Subsequent
	/// keystrokes modify the rope without updating the stored inverse, so the
	/// changeset may not describe the actual edit. Passing an incorrect
	/// changeset to tree-sitter produces a corrupt syntax tree.
	///
	/// When the stored tx correctly transforms `old_source` into the current
	/// content, uses its changeset directly. Otherwise computes a correct
	/// [`Transaction`] via [`rope_delta`] and uses that changeset instead,
	/// keeping the incremental update path even for merged undo groups.
	///
	/// [`rope_delta`]: crate::buffer_sync::convert::rope_delta
	fn incremental_syntax_for_history(
		&mut self,
		old_source: Option<Rope>,
		stored_tx: &Transaction,
		language_loader: &LanguageLoader,
		op: &'static str,
	) {
		let Some(old) = old_source else {
			self.syntax_dirty = true;
			return;
		};
		let mut check = old.clone();
		stored_tx.apply(&mut check);
		if check == self.content {
			self.try_incremental_syntax_update(Some(old), stored_tx.changes(), language_loader, op);
		} else {
			let corrected = crate::buffer_sync::convert::rope_delta(&old, &self.content);
			self.try_incremental_syntax_update(Some(old), corrected.changes(), language_loader, op);
		}
	}

	/// Tries an incremental tree-sitter update; falls back to marking dirty.
	///
	/// On success: clears `syntax_dirty`, bumps `syntax_version`, returns `true`.
	/// On failure or skip: sets `syntax_dirty = true`, returns `false`.
	/// Logs a warning on incremental failure with the given `op` label.
	fn try_incremental_syntax_update(
		&mut self,
		old_source: Option<Rope>,
		changeset: &xeno_primitives::ChangeSet,
		language_loader: &LanguageLoader,
		op: &'static str,
	) -> bool {
		if let Some(old_source) = old_source
			&& let Some(ref mut syntax) = self.syntax
		{
			match syntax.update_from_changeset(
				old_source.slice(..),
				self.content.slice(..),
				changeset,
				language_loader,
				xeno_runtime_language::SyntaxOptions::default(),
			) {
				Ok(()) => {
					self.syntax_dirty = false;
					self.syntax_version = self.syntax_version.wrapping_add(1);
					return true;
				}
				Err(e) => {
					tracing::warn!(error=%e, op, "Incremental syntax update failed");
				}
			}
		}
		self.syntax_dirty = true;
		false
	}

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
			syntax: None,
			syntax_dirty: false,
			insert_undo_active: false,
			version: 0,
			syntax_version: 0,
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
	/// loading is deferred to [`SyntaxManager`] for async background parsing.
	///
	/// [`SyntaxManager`]: crate::syntax_manager::SyntaxManager
	pub fn init_syntax(&mut self, language_loader: &LanguageLoader) {
		if let Some(ref p) = self.path
			&& let Some(lang_id) = language_loader.language_for_path(p)
		{
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.language_id = Some(lang_id);
			self.syntax_version = self.syntax_version.wrapping_add(1);
			// Syntax loading deferred to SyntaxManager - do NOT call Syntax::new here
		}
	}

	/// Initializes syntax highlighting for this document by language name.
	///
	/// This only sets the `language_id` and `file_type` fields. Actual syntax
	/// loading is deferred to [`SyntaxManager`] for async background parsing.
	///
	/// [`SyntaxManager`]: crate::syntax_manager::SyntaxManager
	pub fn init_syntax_for_language(&mut self, name: &str, language_loader: &LanguageLoader) {
		if let Some(lang_id) = language_loader.language_for_name(name) {
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.language_id = Some(lang_id);
			self.syntax_version = self.syntax_version.wrapping_add(1);
			// Syntax loading deferred to SyntaxManager - do NOT call Syntax::new here
		}
	}

	/// Reparses the entire syntax tree from scratch.
	///
	/// Public API entrypoint: schedule a background full reparse.
	pub fn reparse_syntax(&mut self, _language_loader: &LanguageLoader) {
		self.syntax_dirty = true;
		self.syntax_version = self.syntax_version.wrapping_add(1);
	}

	/// Records the current document state as an undo boundary.
	///
	/// Creates a new undo step and ends any active insert grouping session.
	/// Call this before discrete edit operations (delete, change, paste, etc.).
	///
	/// View state (cursor, selection, scroll) is captured separately at the
	/// editor level via [`EditorUndoGroup`].
	///
	/// [`EditorUndoGroup`]: crate::types::EditorUndoGroup
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
	/// Restores document content from the undo stack and reparses syntax.
	/// View state restoration is handled at the editor level via [`EditorUndoGroup`].
	///
	/// Returns the applied inverse transaction on success, or `None` if
	/// nothing to undo.
	///
	/// [`EditorUndoGroup`]: crate::types::EditorUndoGroup
	pub fn undo(&mut self, language_loader: &LanguageLoader) -> Option<Transaction> {
		self.insert_undo_active = false;

		if !self.undo_backend.can_undo() {
			return None;
		}

		let old_source = self.can_sync_incremental().then(|| self.content.clone());

		let tx = self.undo_backend.undo(
			&mut self.content,
			&mut self.version,
			language_loader,
			|_, _| {},
		)?;

		self.incremental_syntax_for_history(old_source, &tx, language_loader, "undo");
		Some(tx)
	}

	/// Redoes the last undone document change.
	///
	/// Restores document content from the redo stack and reparses syntax.
	/// View state restoration is handled at the editor level via [`EditorUndoGroup`].
	///
	/// Returns the applied transaction on success, or `None` if nothing to redo.
	///
	/// [`EditorUndoGroup`]: crate::types::EditorUndoGroup
	pub fn redo(&mut self, language_loader: &LanguageLoader) -> Option<Transaction> {
		self.insert_undo_active = false;

		if !self.undo_backend.can_redo() {
			return None;
		}

		let old_source = self.can_sync_incremental().then(|| self.content.clone());

		let tx = self.undo_backend.redo(
			&mut self.content,
			&mut self.version,
			language_loader,
			|_, _| {},
		)?;

		self.incremental_syntax_for_history(old_source, &tx, language_loader, "redo");
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
	/// - Syntax policy handling
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
	/// Syntax updates are policy-driven:
	/// - [`SyntaxPolicy::None`]: no syntax action
	/// - [`SyntaxPolicy::MarkDirty`]: sets `syntax_dirty` flag for lazy reparse
	/// - [`SyntaxPolicy::IncrementalOrDirty`]: attempts incremental tree-sitter
	///   update using the changeset; falls back to marking dirty on failure
	/// - [`SyntaxPolicy::FullReparseNow`]: immediate full reparse
	///
	/// [`Buffer`]: super::Buffer
	/// [`commit`]: Self::commit
	pub(crate) fn commit_unchecked(
		&mut self,
		commit: EditCommit,
		language_loader: &LanguageLoader,
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

		let old_source_for_syntax = if matches!(commit.syntax, SyntaxPolicy::IncrementalOrDirty)
			&& self.can_sync_incremental()
		{
			Some(self.content.clone())
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
			SyntaxPolicy::MarkDirty => {
				self.syntax_dirty = true;
				SyntaxOutcome::MarkedDirty
			}
			SyntaxPolicy::IncrementalOrDirty => {
				if self.try_incremental_syntax_update(
					old_source_for_syntax,
					commit.tx.changes(),
					language_loader,
					"commit",
				) {
					SyntaxOutcome::IncrementalApplied
				} else {
					SyntaxOutcome::MarkedDirty
				}
			}
			SyntaxPolicy::FullReparseNow => {
				// Never do a blocking full reparse on the commit path.
				self.reparse_syntax(language_loader);
				SyntaxOutcome::MarkedDirty
			}
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
	/// Note: Existing syntax state is preserved but marked dirty, forcing a
	/// full reparse the next time syntax is accessed.
	pub fn reset_content(&mut self, content: impl Into<Rope>) {
		let had_syntax = self.syntax.is_some();
		self.content = content.into();
		self.syntax = None;
		self.syntax_dirty = had_syntax;
		self.insert_undo_active = false;
		self.undo_backend = UndoBackend::default();
		self.modified = false;
		self.version = 0;
	}

	/// Replaces the document content from a cross-process sync snapshot.
	///
	/// Unlike [`reset_content`], this preserves file document semantics:
	/// - Version increments monotonically (not reset to 0).
	/// - Modified flag is set to `true` (content differs from disk).
	/// - Undo history is cleared (undo across ownership boundaries is unsound).
	/// - Syntax state is cleared and marked dirty for reparse.
	///
	/// Use this for buffer sync follower join and full resync, where the remote
	/// content replaces local content but the document still represents a file.
	pub fn install_sync_snapshot(&mut self, content: impl Into<Rope>) {
		self.content = content.into();
		self.syntax = None;
		self.syntax_dirty = true;
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
	/// will be grouped with the current undo step. Used by the Editor layer
	/// to determine whether to push a new [`EditorUndoGroup`].
	///
	/// [`EditorUndoGroup`]: crate::types::EditorUndoGroup
	pub fn insert_undo_active(&self) -> bool {
		self.insert_undo_active
	}

	/// Increments the document version. Called internally during transaction application.
	pub(crate) fn increment_version(&mut self) {
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

	/// Returns whether the document has syntax highlighting enabled.
	pub fn has_syntax(&self) -> bool {
		self.syntax.is_some()
	}

	/// Returns whether the syntax tree needs reparsing.
	///
	/// Set by commits with `SyntaxPolicy::MarkDirty` or `IncrementalOrDirty`.
	pub fn is_syntax_dirty(&self) -> bool {
		self.syntax_dirty
	}

	/// Returns a reference to the syntax highlighting state.
	pub fn syntax(&self) -> Option<&Syntax> {
		self.syntax.as_ref()
	}

	/// Returns a mutable reference to the syntax highlighting state.
	pub fn syntax_mut(&mut self) -> Option<&mut Syntax> {
		self.syntax.as_mut()
	}

	/// Returns the language ID for this document.
	pub fn language_id(&self) -> Option<xeno_runtime_language::LanguageId> {
		self.language_id
	}

	/// Updates the syntax from a completed background parse.
	///
	/// This is the primary entry point for installing a successful parse result.
	/// MUST clear `syntax_dirty`.
	/// MUST bump `syntax_version`.
	pub fn set_syntax(&mut self, syntax: Option<Syntax>) {
		self.syntax = syntax;
		self.syntax_dirty = false;
		self.syntax_version = self.syntax_version.wrapping_add(1);
	}

	/// Write-back for the syntax slot during scheduler integration.
	///
	/// MUST NOT change `syntax_dirty`.
	/// MUST only bump `syntax_version` if `updated` is true.
	pub fn put_syntax_slot(&mut self, syntax: Option<Syntax>, updated: bool) {
		self.syntax = syntax;
		if updated {
			self.syntax_version = self.syntax_version.wrapping_add(1);
		}
	}

	/// Marks syntax as dirty (needing reparse).
	pub fn mark_syntax_dirty(&mut self) {
		self.syntax_dirty = true;
	}

	/// Clears the syntax dirty flag.
	pub fn clear_syntax_dirty(&mut self) {
		self.syntax_dirty = false;
	}

	/// Takes the syntax out of the document, leaving `None`.
	pub fn take_syntax(&mut self) -> Option<Syntax> {
		self.syntax.take()
	}

	/// Resets the insert undo grouping flag.
	pub(crate) fn reset_insert_undo(&mut self) {
		self.insert_undo_active = false;
	}
}

fn collect_changed_ranges(tx: &xeno_primitives::Transaction) -> Vec<Range> {
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
