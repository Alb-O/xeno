//! Document - the shared, file-backed content of a buffer.
//!
//! A [`Document`] represents the actual file content, separate from any view state.
//! Multiple buffers can reference the same document, enabling split views of
//! the same file with shared undo history.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use evildoer_base::{Rope, Selection};
use evildoer_language::LanguageLoader;
use evildoer_language::syntax::Syntax;

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
pub struct Document {
	/// Unique identifier for this document.
	pub id: DocumentId,

	/// The text content.
	pub content: Rope,

	/// Associated file path (None for scratch documents).
	pub path: Option<PathBuf>,

	/// Whether the document has unsaved changes.
	pub modified: bool,

	/// Undo history stack.
	pub undo_stack: Vec<HistoryEntry>,

	/// Redo history stack.
	pub redo_stack: Vec<HistoryEntry>,

	/// Detected file type (e.g., "rust", "python").
	pub file_type: Option<String>,

	/// Syntax highlighting state.
	pub syntax: Option<Syntax>,

	/// Flag for grouping insert-mode edits into a single undo.
	pub(crate) insert_undo_active: bool,

	/// Document version, incremented on every transaction.
	///
	/// Used for LSP synchronization and cache invalidation.
	pub version: u64,
}

impl Document {
	/// Creates a new document with the given content and optional file path.
	pub fn new(content: String, path: Option<PathBuf>) -> Self {
		Self {
			id: DocumentId::next(),
			content: Rope::from(content.as_str()),
			path,
			modified: false,
			undo_stack: Vec::new(),
			redo_stack: Vec::new(),
			file_type: None,
			syntax: None,
			insert_undo_active: false,
			version: 0,
		}
	}

	/// Creates a new scratch document (no file path).
	pub fn scratch() -> Self {
		Self::new(String::new(), None)
	}

	/// Initializes syntax highlighting for this document.
	pub fn init_syntax(&mut self, language_loader: &LanguageLoader) {
		if let Some(ref p) = self.path
			&& let Some(lang_id) = language_loader.language_for_path(p)
		{
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
	///
	/// The selection is passed in because it belongs to the view making the edit.
	pub(crate) fn push_undo_snapshot(&mut self, selection: &Selection) {
		self.undo_stack.push(HistoryEntry {
			doc: self.content.clone(),
			selection: selection.clone(),
		});
		self.redo_stack.clear();

		const MAX_UNDO: usize = 100;
		if self.undo_stack.len() > MAX_UNDO {
			self.undo_stack.remove(0);
		}
	}

	/// Saves current state to undo history.
	///
	/// Explicit calls reset any grouped insert session.
	pub fn save_undo_state(&mut self, selection: &Selection) {
		self.insert_undo_active = false;
		self.push_undo_snapshot(selection);
	}

	/// Saves undo state for insert mode, grouping consecutive inserts.
	///
	/// Returns true if a new snapshot was created.
	pub fn save_insert_undo_state(&mut self, selection: &Selection) -> bool {
		if self.insert_undo_active {
			return false;
		}
		self.insert_undo_active = true;
		self.push_undo_snapshot(selection);
		true
	}

	/// Undoes the last change.
	///
	/// Returns the restored selection if successful, or None if nothing to undo.
	pub fn undo(&mut self, language_loader: &LanguageLoader) -> Option<Selection> {
		self.insert_undo_active = false;
		if let Some(current_selection) = self.undo_stack.last().map(|e| e.selection.clone()) {
			// Save current state to redo before popping
			if let Some(entry) = self.undo_stack.pop() {
				self.redo_stack.push(HistoryEntry {
					doc: self.content.clone(),
					selection: current_selection,
				});

				self.content = entry.doc;
				self.reparse_syntax(language_loader);
				return Some(entry.selection);
			}
		}
		None
	}

	/// Redoes the last undone change.
	///
	/// Returns the restored selection if successful, or None if nothing to redo.
	pub fn redo(
		&mut self,
		current_selection: &Selection,
		language_loader: &LanguageLoader,
	) -> Option<Selection> {
		self.insert_undo_active = false;
		if let Some(entry) = self.redo_stack.pop() {
			self.undo_stack.push(HistoryEntry {
				doc: self.content.clone(),
				selection: current_selection.clone(),
			});

			self.content = entry.doc;
			self.reparse_syntax(language_loader);
			Some(entry.selection)
		} else {
			None
		}
	}
}
