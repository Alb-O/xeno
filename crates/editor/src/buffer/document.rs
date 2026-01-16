//! Document - the shared, file-backed content of a buffer.
//!
//! A [`Document`] represents the actual file content, separate from any view state.
//! Multiple buffers can reference the same document, enabling split views of
//! the same file with shared undo history.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use xeno_primitives::{
	CommitResult, EditCommit, EditError, ReadOnlyReason, ReadOnlyScope, Rope, Selection,
	SyntaxPolicy, UndoPolicy,
};
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::syntax::Syntax;

use crate::buffer::BufferId;
use crate::editor::types::HistoryEntry;

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
/// Undo history is per-document, not per-view. This means undoing in one view
/// affects all views of the same document. The selection state stored in
/// history entries is from the view that made the edit.
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

	/// Undo history stack.
	undo_stack: Vec<HistoryEntry>,

	/// Redo history stack.
	redo_stack: Vec<HistoryEntry>,

	/// Detected file type (e.g., "rust", "python").
	pub file_type: Option<String>,

	/// Syntax highlighting state.
	syntax: Option<Syntax>,

	/// Flag for grouping insert-mode edits into a single undo.
	insert_undo_active: bool,

	/// Document version, incremented on every transaction.
	///
	/// Used for LSP synchronization and cache invalidation.
	version: u64,

	/// Pending LSP changes queued for sync.
	#[cfg(feature = "lsp")]
	pending_lsp_changes: Vec<xeno_primitives::LspDocumentChange>,
	/// Force a full LSP sync on the next flush.
	#[cfg(feature = "lsp")]
	force_full_sync: bool,
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
			undo_stack: Vec::new(),
			redo_stack: Vec::new(),
			file_type: None,
			syntax: None,
			insert_undo_active: false,
			version: 0,
			#[cfg(feature = "lsp")]
			pending_lsp_changes: Vec::new(),
			#[cfg(feature = "lsp")]
			force_full_sync: false,
		}
	}

	/// Creates a new scratch document (no file path).
	pub fn scratch() -> Self {
		Self::new(String::new(), None)
	}

	/// Initializes syntax highlighting for this document based on file path.
	pub fn init_syntax(&mut self, language_loader: &LanguageLoader) {
		if let Some(ref p) = self.path
			&& let Some(lang_id) = language_loader.language_for_path(p)
		{
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.syntax = Syntax::new(self.content.slice(..), lang_id, language_loader).ok();
		}
	}

	/// Initializes syntax highlighting for this document by language name.
	pub fn init_syntax_for_language(&mut self, name: &str, language_loader: &LanguageLoader) {
		if let Some(lang_id) = language_loader.language_for_name(name) {
			let lang_data = language_loader.get(lang_id);
			self.file_type = lang_data.map(|l| l.name.clone());
			self.syntax = Syntax::new(self.content.slice(..), lang_id, language_loader).ok();
		}
	}

	/// Reparses the entire syntax tree from scratch.
	///
	/// Used after operations that replace the entire document (undo/redo).
	pub fn reparse_syntax(&mut self, language_loader: &LanguageLoader) {
		if self.syntax.is_some() {
			let lang_id = self.syntax.as_ref().unwrap().root_language();
			self.syntax = Syntax::new(self.content.slice(..), lang_id, language_loader).ok();
		}
	}

	/// Pushes the current state onto the undo stack.
	pub(crate) fn push_undo_snapshot(&mut self, selections: HashMap<BufferId, Selection>) {
		self.undo_stack.push(HistoryEntry {
			doc: self.content.clone(),
			selections,
		});
		self.redo_stack.clear();

		const MAX_UNDO: usize = 100;
		if self.undo_stack.len() > MAX_UNDO {
			self.undo_stack.remove(0);
		}
	}

	/// Saves current state to undo history. Resets any grouped insert session.
	pub fn save_undo_state(&mut self, selections: HashMap<BufferId, Selection>) {
		self.insert_undo_active = false;
		self.push_undo_snapshot(selections);
	}

	/// Saves undo state for insert mode, grouping consecutive inserts.
	/// Returns true if a new snapshot was created.
	pub fn save_insert_undo_state(&mut self, selections: HashMap<BufferId, Selection>) -> bool {
		if self.insert_undo_active {
			return false;
		}
		self.insert_undo_active = true;
		self.push_undo_snapshot(selections);
		true
	}

	/// Undoes the last change. Returns restored selections if successful.
	pub fn undo(
		&mut self,
		current_selections: HashMap<BufferId, Selection>,
		language_loader: &LanguageLoader,
	) -> Option<HashMap<BufferId, Selection>> {
		self.insert_undo_active = false;
		let entry = self.undo_stack.pop()?;
		self.redo_stack.push(HistoryEntry {
			doc: self.content.clone(),
			selections: current_selections,
		});
		self.content = entry.doc;
		self.reparse_syntax(language_loader);
		Some(entry.selections)
	}

	/// Redoes the last undone change. Returns restored selections if successful.
	pub fn redo(
		&mut self,
		current_selections: HashMap<BufferId, Selection>,
		language_loader: &LanguageLoader,
	) -> Option<HashMap<BufferId, Selection>> {
		self.insert_undo_active = false;
		let entry = self.redo_stack.pop()?;
		self.undo_stack.push(HistoryEntry {
			doc: self.content.clone(),
			selections: current_selections,
		});
		self.content = entry.doc;
		self.reparse_syntax(language_loader);
		Some(entry.selections)
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
	/// # Arguments
	///
	/// * `commit` - The edit commit containing transaction and policies
	/// * `selections` - Current selections for undo snapshot (temporary for Phase 3;
	///   will be removed when document/view history is separated in Phase 4)
	/// * `language_loader` - For syntax reparsing if needed
	///
	/// # Errors
	///
	/// Returns `EditError::ReadOnly` if the document is readonly.
	pub fn commit(
		&mut self,
		commit: EditCommit,
		selections: HashMap<BufferId, Selection>,
		language_loader: &LanguageLoader,
	) -> Result<CommitResult, EditError> {
		self.ensure_writable()?;
		Ok(self.commit_unchecked(commit, selections, language_loader))
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
	/// Syntax updates use full reparse for all non-`None` policies (incremental
	/// updates will be added in Phase 6).
	///
	/// [`Buffer`]: super::Buffer
	/// [`commit`]: Self::commit
	pub(crate) fn commit_unchecked(
		&mut self,
		commit: EditCommit,
		selections: HashMap<BufferId, Selection>,
		language_loader: &LanguageLoader,
	) -> CommitResult {
		let version_before = self.version;

		let undo_recorded = match commit.undo {
			UndoPolicy::NoUndo => false,
			UndoPolicy::Record | UndoPolicy::Boundary => {
				self.insert_undo_active = false;
				self.push_undo_snapshot(selections);
				true
			}
			UndoPolicy::MergeWithCurrentGroup => {
				if !self.insert_undo_active {
					self.insert_undo_active = true;
					self.push_undo_snapshot(selections);
					true
				} else {
					false
				}
			}
		};

		commit.tx.apply(&mut self.content);
		self.modified = true;
		self.version = self.version.wrapping_add(1);

		let syntax_changed = match commit.syntax {
			SyntaxPolicy::None => false,
			SyntaxPolicy::FullReparseNow
			| SyntaxPolicy::MarkDirty
			| SyntaxPolicy::IncrementalOrDirty => {
				self.reparse_syntax(language_loader);
				true
			}
		};

		CommitResult {
			applied: true,
			version_before,
			version_after: self.version,
			selection_after: commit.selection_after,
			syntax_changed,
			undo_recorded,
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
	/// This is a low-level accessor. Prefer using `commit()` (when available)
	/// or transaction-based methods that properly handle undo/syntax updates.
	pub fn content_mut(&mut self) -> &mut Rope {
		&mut self.content
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

	/// Increments the document version. Called internally during transaction application.
	pub(crate) fn increment_version(&mut self) {
		self.version = self.version.wrapping_add(1);
	}

	/// Returns the number of items in the undo stack.
	pub fn undo_len(&self) -> usize {
		self.undo_stack.len()
	}

	/// Returns the number of items in the redo stack.
	pub fn redo_len(&self) -> usize {
		self.redo_stack.len()
	}

	/// Returns whether undo is available.
	pub fn can_undo(&self) -> bool {
		!self.undo_stack.is_empty()
	}

	/// Returns whether redo is available.
	pub fn can_redo(&self) -> bool {
		!self.redo_stack.is_empty()
	}

	/// Returns whether the document has syntax highlighting enabled.
	pub fn has_syntax(&self) -> bool {
		self.syntax.is_some()
	}

	/// Returns a reference to the syntax highlighting state.
	pub fn syntax(&self) -> Option<&Syntax> {
		self.syntax.as_ref()
	}

	/// Returns a mutable reference to the syntax highlighting state.
	pub fn syntax_mut(&mut self) -> Option<&mut Syntax> {
		self.syntax.as_mut()
	}

	/// Resets the insert undo grouping flag.
	pub(crate) fn reset_insert_undo(&mut self) {
		self.insert_undo_active = false;
	}

	/// Returns whether there are pending LSP changes or a full sync is required.
	#[cfg(feature = "lsp")]
	pub fn has_pending_lsp_sync(&self) -> bool {
		self.force_full_sync || !self.pending_lsp_changes.is_empty()
	}

	/// Returns whether a full LSP sync is required.
	#[cfg(feature = "lsp")]
	pub fn needs_full_lsp_sync(&self) -> bool {
		self.force_full_sync
	}

	/// Marks that a full LSP sync is required.
	#[cfg(feature = "lsp")]
	pub fn mark_for_full_lsp_sync(&mut self) {
		self.force_full_sync = true;
		self.pending_lsp_changes.clear();
	}

	/// Clears the full sync flag (called after performing full sync).
	#[cfg(feature = "lsp")]
	pub fn clear_full_lsp_sync(&mut self) {
		self.force_full_sync = false;
	}

	/// Appends LSP changes to the pending queue.
	#[cfg(feature = "lsp")]
	pub fn extend_lsp_changes(&mut self, changes: Vec<xeno_primitives::LspDocumentChange>) {
		self.pending_lsp_changes.extend(changes);
	}

	/// Drains and returns all pending LSP changes.
	#[cfg(feature = "lsp")]
	pub fn drain_lsp_changes(&mut self) -> Vec<xeno_primitives::LspDocumentChange> {
		std::mem::take(&mut self.pending_lsp_changes)
	}
}
