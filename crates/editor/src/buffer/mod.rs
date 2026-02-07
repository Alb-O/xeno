//! Buffer - the core text editing unit.
//!
//! # Purpose
//!
//! - Owns: per-view state (cursor, selection, scroll position, local options) and modal input state.
//! - Does not own: authoritative document content (owned by [`crate::core::document::Document`]).
//! - Source of truth: [`crate::buffer::Buffer`].
//!
//! # Mental model
//!
//! - A buffer is a view into a document.
//! - Multiple buffers can point to the same document (enabling splits).
//! - View-local state (like the cursor) is stored in the buffer.
//! - Shared state (like text and history) is stored in the document.
//! - Thread-safety for shared documents is managed via [`crate::buffer::DocumentHandle`] with re-entrancy protection.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`crate::buffer::Buffer`] | Primary editing unit | MUST separate view state from content | `Buffer::new`, `Buffer::clone_for_split` |
//! | [`crate::core::document::Document`] | Shared content | Authoritative source of text/history | `Document::new` |
//! | [`crate::buffer::DocumentHandle`] | Thread-safe wrapper | MUST prevent re-entrant locks on same thread | `DocumentHandle::new` |
//! | [`crate::buffer::ApplyPolicy`] | Edit validation rules | Controls readonly/history behavior | `editing::apply` |
//!
//! # Invariants
//!
//! - Must not allow re-entrant locking of the same document on a single thread.
//! - Must keep view state (cursor/selection) within document bounds.
//! - Must preserve monotonic document versions across edits.
//!
//! # Data flow
//!
//! 1. Input: User keys flow into [`InputHandler`].
//! 2. Resolution: Input produces an action which calls `Buffer` methods.
//! 3. Mutation: `Buffer` calls [`DocumentHandle::with_mut`] to apply edits.
//! 4. Notification: Document changes trigger version bumps and event emission.
//!
//! # Concurrency and ordering
//!
//! - Multi-view consistency: Edits to a shared document are immediately visible to all buffers.
//! - Lock ordering: Always acquire document locks for the shortest possible duration.
//! - Thread-safety: `Document` is wrapped in `Arc<RwLock<Document>>` inside `DocumentHandle`.
//!
//! # Failure modes and recovery
//!
//! - Readonly violation: Edits to readonly documents/buffers return `EditError`.
//! - Deadlock prevention: Re-entrant lock attempts trigger a controlled panic via `LockGuard`.
//!
//! # Recipes
//!
//! ## Split a view
//!
//! - Call `buffer.clone_for_split(new_view_id)`.
//! - This creates a new buffer sharing the same `DocumentHandle`.
//!
//! ## Apply an edit
//!
//! - Use `buffer.apply_transaction(tx, policy)`.
//! - This handles versioning, history, and readonly checks automatically.
//!
mod editing;

mod layout;
mod navigation;

#[cfg(test)]
mod invariants;

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

pub use editing::ApplyPolicy;
pub use layout::{Layout, SpatialDirection, SplitDirection, SplitPath};
use parking_lot::RwLock;
use xeno_input::input::InputHandler;
pub use xeno_primitives::ViewId;
use xeno_primitives::range::CharIdx;
use xeno_primitives::{Mode, Selection};
use xeno_registry::options::{
	FromOptionValue, OptionKey, OptionStore, OptionValue, TypedOptionKey,
};
use xeno_runtime_language::LanguageLoader;

pub use crate::core::document::{Document, DocumentId, DocumentMetaOutcome};
pub use crate::core::history::HistoryResult;
pub use crate::core::undo_store::{TxnUndoStore, UndoBackend};

/// Capability token that gates access to [`Document::commit_unchecked`].
///
/// The constructor is private to the `buffer` module tree, so only `Buffer`
/// code can create one. This prevents other modules from calling the
/// unchecked commit path even though it is `pub(crate)`.
pub(crate) struct CommitBypassToken(());

impl CommitBypassToken {
	fn new() -> Self {
		Self(())
	}
}

// Thread-local set of document IDs currently locked by the thread.
// Used to detect and prevent re-entrant locking on the same document,
// which would cause a self-deadlock. Enabled in all builds for reliability.
thread_local! {
	static ACTIVE_DOC_LOCKS: RefCell<HashSet<DocumentId>> = RefCell::new(HashSet::new());
}

/// A handle to a shared [`Document`], managing thread-safe access.
///
/// Wraps an `Arc<RwLock<Document>>` and provides scoped access via closures.
/// It enforces a strict no-reentrancy policy per document, keyed by [`DocumentId`].
#[derive(Clone)]
pub(crate) struct DocumentHandle {
	/// Stable document identity for lock-free reentrancy checks.
	doc_id: DocumentId,
	/// Shared pointer to the authoritative document state.
	inner: Arc<RwLock<Document>>,
}

impl DocumentHandle {
	/// Creates a new handle for the given document.
	fn new(document: Document) -> Self {
		let doc_id = document.id;
		let inner = Arc::new(RwLock::new(document));
		Self { doc_id, inner }
	}

	/// Executes a closure with shared (read) access to the document.
	///
	/// # Panics
	///
	/// Panics if the current thread already holds a lock on this specific document.
	#[track_caller]
	fn with<R>(&self, f: impl FnOnce(&Document) -> R) -> R {
		let _guard = LockGuard::new(self.doc_id);
		let guard = self.inner.read();
		f(&guard)
	}

	/// Executes a closure with exclusive (write) access to the document.
	///
	/// # Panics
	///
	/// Panics if the current thread already holds a lock on this specific document.
	#[track_caller]
	fn with_mut<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
		let _guard = LockGuard::new(self.doc_id);
		let mut guard = self.inner.write();
		f(&mut guard)
	}

	/// Checks if two handles point to the same underlying document.
	fn ptr_eq(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.inner, &other.inner)
	}
}

/// RAII guard for tracking active document locks on the current thread.
struct LockGuard(DocumentId);

impl LockGuard {
	/// Registers a lock on the given document.
	///
	/// # Panics
	///
	/// Panics if the document is already locked by the current thread.
	#[track_caller]
	fn new(doc_id: DocumentId) -> Self {
		ACTIVE_DOC_LOCKS.with(|locks| {
			let mut locks = locks.borrow_mut();
			if locks.contains(&doc_id) {
				panic!(
					"Deadlock detected: re-entrant lock on DocumentId({})",
					doc_id.0
				);
			}
			locks.insert(doc_id);
		});
		Self(doc_id)
	}
}

impl Drop for LockGuard {
	fn drop(&mut self) {
		ACTIVE_DOC_LOCKS.with(|locks| {
			locks.borrow_mut().remove(&self.0);
		});
	}
}

/// A text buffer combining a document view with local view state.
///
/// Provides access to both view state (cursor, selection, scroll) and
/// shared document content. Multiple buffers can share the same underlying
/// [`Document`], enabling synchronized split views.
pub struct Buffer {
	/// Unique identifier for this view.
	pub id: ViewId,
	/// The underlying document.
	document: DocumentHandle,
	/// Primary cursor position (char index).
	pub cursor: CharIdx,
	/// Multi-cursor selection state.
	pub selection: Selection,
	/// Modal input handler tracking mode and pending sequences.
	pub input: InputHandler,
	/// Scroll position: first visible line index.
	pub scroll_line: usize,
	/// Scroll position: horizontal segment index (for wrapped lines).
	pub scroll_segment: usize,
	/// Text width used for wrapping calculations.
	pub text_width: usize,
	/// Viewport height observed during the last render pass.
	pub last_viewport_height: usize,
	/// Cursor position observed during the last render pass.
	pub last_rendered_cursor: CharIdx,
	/// If true, suppresses automatic viewport adjustments to keep the cursor visible.
	pub suppress_auto_scroll: bool,
	/// Buffer-local option overrides.
	pub local_options: OptionStore,
	/// Optional read-only override for this specific view.
	readonly_override: Option<bool>,
	/// Remembered column for vertical navigation (j/k) stability.
	goal_column: Option<usize>,
}

impl Buffer {
	/// Creates a new buffer with the given ID and content.
	pub fn new(id: ViewId, content: String, path: Option<PathBuf>) -> Self {
		let document = DocumentHandle::new(Document::new(content, path));
		Self {
			id,
			document,
			cursor: 0,
			selection: Selection::point(0),
			input: InputHandler::new(),
			scroll_line: 0,
			scroll_segment: 0,
			text_width: 80,
			last_viewport_height: 0,
			last_rendered_cursor: 0,
			suppress_auto_scroll: false,
			local_options: OptionStore::new(),
			readonly_override: None,
			goal_column: None,
		}
	}

	/// Creates a new scratch buffer with no file path.
	pub fn scratch(id: ViewId) -> Self {
		Self::new(id, String::new(), None)
	}

	/// Creates a new buffer that shares the same document (for split views).
	///
	/// The new buffer has independent view state (cursor, scroll, options) but
	/// share the authoritative document content and history.
	pub fn clone_for_split(&self, new_id: ViewId) -> Self {
		Self {
			id: new_id,
			document: self.document.clone(),
			cursor: self.cursor,
			selection: self.selection.clone(),
			input: InputHandler::new(),
			scroll_line: self.scroll_line,
			scroll_segment: self.scroll_segment,
			text_width: self.text_width,
			last_viewport_height: 0,
			last_rendered_cursor: self.cursor,
			suppress_auto_scroll: false,
			local_options: self.local_options.clone(),
			readonly_override: None,
			goal_column: None,
		}
	}

	pub fn document_id(&self) -> DocumentId {
		self.document.doc_id
	}

	/// Checks if this buffer shares a document with another buffer.
	pub fn shares_document_with(&self, other: &Buffer) -> bool {
		self.document.ptr_eq(&other.document)
	}

	/// Executes a closure with read access to the underlying [`Document`].
	#[inline]
	pub fn with_doc<R>(&self, f: impl FnOnce(&Document) -> R) -> R {
		self.document.with(f)
	}

	/// Executes a closure with write access to the underlying [`Document`].
	#[inline]
	pub fn with_doc_mut<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
		self.document.with_mut(f)
	}

	pub fn path(&self) -> Option<PathBuf> {
		self.with_doc(|doc| doc.path().cloned())
	}

	pub fn set_path(
		&mut self,
		path: Option<PathBuf>,
		loader: Option<&LanguageLoader>,
	) -> DocumentMetaOutcome {
		self.with_doc_mut(|doc| doc.set_path(path, loader))
	}

	pub fn modified(&self) -> bool {
		self.with_doc(|doc| doc.is_modified())
	}

	pub fn set_modified(&mut self, modified: bool) -> DocumentMetaOutcome {
		self.with_doc_mut(|doc| doc.set_modified(modified))
	}

	/// Returns whether this buffer is read-only.
	///
	/// The override is additive-only: `Some(true)` forces read-only regardless
	/// of the document flag, but `Some(false)` or `None` both defer to the
	/// document's readonly state.
	pub fn is_readonly(&self) -> bool {
		self.readonly_override == Some(true) || self.with_doc(|doc| doc.is_readonly())
	}

	pub fn set_readonly(&mut self, readonly: bool) -> DocumentMetaOutcome {
		self.with_doc_mut(|doc| doc.set_readonly(readonly))
	}

	/// Sets a buffer-level readonly override.
	///
	/// The override is additive-only:
	/// - `Some(true)`: Force read-only regardless of document state.
	/// - `None`: Defer to the document's readonly flag.
	///
	/// `Some(false)` is treated identically to `None` (it cannot bypass
	/// document-level readonly).
	pub fn set_readonly_override(&mut self, readonly: Option<bool>) {
		self.readonly_override = if readonly == Some(true) {
			Some(true)
		} else {
			None
		};
	}

	/// Replaces the document content wholesale, clearing history.
	pub fn reset_content(&mut self, content: impl Into<xeno_primitives::Rope>) {
		self.with_doc_mut(|doc| doc.reset_content(content));
	}

	/// Clears the active undo group owner for the underlying document.
	pub fn clear_undo_group(&mut self) {
		self.with_doc_mut(|doc| doc.clear_undo_group());
	}

	pub fn version(&self) -> u64 {
		self.with_doc(|doc| doc.version())
	}

	pub fn file_type(&self) -> Option<String> {
		self.with_doc(|doc| doc.file_type().map(String::from))
	}

	/// Initializes language metadata for this buffer.
	pub fn init_syntax(&mut self, language_loader: &LanguageLoader) {
		self.with_doc_mut(|doc| doc.init_syntax(language_loader));
	}

	pub fn mode(&self) -> Mode {
		self.input.mode()
	}

	pub fn mode_name(&self) -> &'static str {
		self.input.mode_name()
	}

	/// Returns the line number containing the cursor.
	pub fn cursor_line(&self) -> usize {
		self.with_doc(|doc| self.cursor_line_with_doc(doc))
	}

	#[inline]
	pub(crate) fn cursor_line_with_doc(&self, doc: &Document) -> usize {
		let text = doc.content();
		text.char_to_line(self.cursor.min(text.len_chars()))
	}

	/// Returns the column of the cursor within its line.
	pub fn cursor_col(&self) -> usize {
		self.with_doc(|doc| self.cursor_col_with_doc(doc))
	}

	#[inline]
	pub(crate) fn cursor_col_with_doc(&self, doc: &Document) -> usize {
		let text = doc.content();
		let line = text.char_to_line(self.cursor.min(text.len_chars()));
		self.cursor.saturating_sub(text.line_to_char(line))
	}

	/// Computes the combined width of all enabled gutter columns.
	pub fn gutter_width(&self) -> u16 {
		self.with_doc(|doc| self.gutter_width_with_doc(doc))
	}

	#[inline]
	pub(crate) fn gutter_width_with_doc(&self, doc: &Document) -> u16 {
		use xeno_registry::gutter::{GutterWidthContext, total_width};

		let ctx = GutterWidthContext {
			total_lines: doc.content().len_lines(),
			viewport_width: self.text_width as u16,
		};
		total_width(&ctx)
	}

	pub fn undo_stack_len(&self) -> usize {
		self.with_doc(|doc| doc.undo_len())
	}

	pub fn redo_stack_len(&self) -> usize {
		self.with_doc(|doc| doc.redo_len())
	}

	/// Clamps selection and cursor to valid document bounds.
	pub fn ensure_valid_selection(&mut self) {
		let max_char = self.with_doc(|doc| doc.content().len_chars());
		self.selection.clamp(max_char);
		self.cursor = self.cursor.min(max_char);
	}

	/// Asserts that selection and cursor are within valid document bounds.
	#[cfg(debug_assertions)]
	pub fn debug_assert_valid_state(&self) {
		self.with_doc(|doc| {
			let len = doc.content().len_chars();
			debug_assert!(
				self.selection.is_in_bounds(len),
				"selection out of bounds: len={}, selection={:?}",
				len,
				self.selection
			);
			debug_assert!(
				self.cursor <= len,
				"cursor out of bounds: cursor={}, len={}",
				self.cursor,
				len
			);
		});
	}

	#[cfg(not(debug_assertions))]
	#[inline]
	pub fn debug_assert_valid_state(&self) {}

	/// Maps selection and cursor through a transaction delta.
	pub fn map_selection_through(&mut self, tx: &xeno_primitives::Transaction) {
		self.set_selection(tx.map_selection(&self.selection));
		self.sync_cursor_to_selection();
	}

	/// Resolves an option for this buffer using the layered configuration system.
	pub fn option_raw(&self, key: OptionKey, editor: &crate::impls::Editor) -> OptionValue {
		editor.resolve_option(self.id, key)
	}

	/// Resolves a typed option for this buffer.
	pub fn option<T: FromOptionValue>(
		&self,
		key: TypedOptionKey<T>,
		editor: &crate::impls::Editor,
	) -> T {
		T::from_option(&self.option_raw(key.untyped(), editor))
			.or_else(|| T::from_option(&key.def().default.to_value()))
			.expect("option type mismatch with registered default")
	}

	/// Sets cursor position and resets goal column.
	#[inline]
	pub fn set_cursor(&mut self, pos: CharIdx) {
		self.cursor = pos;
		self.goal_column = None;
	}

	/// Sets selection and resets goal column.
	#[inline]
	pub fn set_selection(&mut self, sel: Selection) {
		self.selection = sel;
		self.goal_column = None;
	}

	/// Syncs cursor to the selection head without resetting goal column.
	#[inline]
	pub fn sync_cursor_to_selection(&mut self) {
		self.cursor = self.selection.primary().head;
	}

	/// Sets both cursor and selection, resetting goal column.
	#[inline]
	pub fn set_cursor_and_selection(&mut self, pos: CharIdx, sel: Selection) {
		self.cursor = pos;
		self.selection = sel;
		self.goal_column = None;
	}

	/// Maintains the horizontal position (goal column) during vertical movement.
	#[inline]
	pub fn establish_goal_column(&mut self) {
		let cursor = self.cursor;
		self.goal_column = Some(self.with_doc(|doc| {
			let text = doc.content();
			let line = text.char_to_line(cursor.min(text.len_chars()));
			cursor.saturating_sub(text.line_to_char(line))
		}));
	}
}
