//! Buffer - the core text editing unit.
//!
//! The buffer system separates document content from view state:
//! - [`Document`] holds shared content (text, undo history, syntax)
//! - [`Buffer`] holds per-view state (cursor, selection, scroll position)
//!
//! Multiple buffers can share the same document, enabling proper split behavior.

pub mod document;
mod editing;
mod history;
mod layout;
mod navigation;
mod undo_store;

use std::path::PathBuf;
use std::sync::Arc;

pub use document::{Document, DocumentId};
pub use editing::ApplyPolicy;
pub use history::HistoryResult;
pub use layout::{Layout, SpatialDirection, SplitDirection, SplitPath};
use parking_lot::RwLock;
pub use undo_store::{TxnUndoStore, UndoBackend};
pub use xeno_primitives::ViewId;
use xeno_primitives::range::CharIdx;
use xeno_primitives::{Mode, Selection};
use xeno_registry::options::{
	FromOptionValue, OptionKey, OptionStore, OptionValue, TypedOptionKey,
};
use xeno_runtime_language::LanguageLoader;

use crate::input::InputHandler;

#[derive(Clone)]
pub(crate) struct DocumentHandle(Arc<RwLock<Document>>);

impl DocumentHandle {
	fn new(document: Document) -> Self {
		Self(Arc::new(RwLock::new(document)))
	}

	fn with<R>(&self, f: impl FnOnce(&Document) -> R) -> R {
		let guard = self.0.read();
		f(&guard)
	}

	fn with_mut<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
		let mut guard = self.0.write();
		f(&mut guard)
	}

	fn ptr_eq(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.0, &other.0)
	}
}

/// A text buffer - combines a view with its document.
///
/// Provides access to both view state (cursor, selection, scroll) and
/// document state (content, undo history, metadata).
///
/// For split views, multiple Buffers can share the same underlying Document.
pub struct Buffer {
	/// Unique identifier for this buffer/view.
	pub id: ViewId,

	/// The underlying document (shared across split views).
	document: DocumentHandle,

	/// Primary cursor position (char index).
	pub cursor: CharIdx,

	/// Multi-cursor selection state.
	pub selection: Selection,

	/// Modal input handler (tracks mode, pending keys, count).
	pub input: InputHandler,

	/// Scroll position: first visible line.
	pub scroll_line: usize,

	/// Scroll position: first visible segment within the line (for wrapped lines).
	pub scroll_segment: usize,

	/// Text width for wrapping calculations.
	pub text_width: usize,

	/// Last rendered viewport height (in rows).
	pub last_viewport_height: usize,

	/// Cursor position observed during the last render.
	pub last_rendered_cursor: CharIdx,

	/// Suppresses automatic viewport adjustment to keep cursor visible.
	///
	/// When set, [`ensure_buffer_cursor_visible`] skips viewport adjustment,
	/// allowing the cursor to be outside the visible area. Used during mouse
	/// scrolling and split resizing for viewport stability. Cleared on cursor move.
	///
	/// [`ensure_buffer_cursor_visible`]: crate::render::buffer::viewport::ensure_buffer_cursor_visible
	pub suppress_auto_scroll: bool,

	/// Buffer-local option overrides (set via `:setlocal`).
	///
	/// These take precedence over language-specific and global options when
	/// resolving option values for this buffer.
	pub local_options: OptionStore,

	/// Buffer-level readonly override.
	///
	/// When `Some(true)`, this buffer is read-only regardless of the underlying
	/// document's readonly state. When `Some(false)`, this buffer is writable
	/// even if the document is marked readonly. When `None`, defers to the
	/// document's readonly flag.
	///
	/// This enables read-only views (e.g., info popups, documentation panels)
	/// without affecting other buffers sharing the same document.
	readonly_override: Option<bool>,

	/// Remembered column position for vertical navigation.
	///
	/// When moving vertically (j/k, up/down, scroll), the cursor should return
	/// to this column when reaching lines long enough to accommodate it. This
	/// prevents the cursor from drifting left when crossing short or empty lines.
	///
	/// Set when vertical motion begins from current cursor column. Reset when
	/// any horizontal or explicit cursor movement occurs (h/l, word motions,
	/// goto, mouse click, etc.).
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

	/// Creates a new scratch buffer.
	pub fn scratch(id: ViewId) -> Self {
		Self::new(id, String::new(), None)
	}

	/// Creates a new buffer that shares the same document (for split views).
	///
	/// The new buffer has independent cursor/selection/scroll state but
	/// edits in either buffer affect both. Local options are cloned so each
	/// split can have independent option overrides. The readonly override is
	/// intentionally NOT cloned - splits start with no override (deferring to
	/// the document's readonly state).
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

	/// Returns the document ID.
	pub fn document_id(&self) -> DocumentId {
		self.with_doc(|doc| doc.id)
	}

	/// Checks if this buffer shares a document with another buffer.
	pub fn shares_document_with(&self, other: &Buffer) -> bool {
		self.document.ptr_eq(&other.document)
	}

	/// Executes a closure with read access to the underlying [`Document`].
	///
	/// This is the preferred API for document access as it ensures the lock
	/// guard cannot escape the scope, preventing potential deadlocks.
	///
	/// # Examples
	///
	/// ```ignore
	/// let line_count = buffer.with_doc(|doc| doc.content().len_lines());
	/// ```
	#[inline]
	pub fn with_doc<R>(&self, f: impl FnOnce(&Document) -> R) -> R {
		self.document.with(f)
	}

	/// Executes a closure with write access to the underlying [`Document`].
	///
	/// This is the preferred API for document mutation as it ensures the lock
	/// guard cannot escape the scope, preventing potential deadlocks.
	///
	/// # Examples
	///
	/// ```ignore
	/// buffer.with_doc_mut(|doc| {
	///     doc.set_modified(true);
	/// });
	/// ```
	#[inline]
	pub fn with_doc_mut<R>(&self, f: impl FnOnce(&mut Document) -> R) -> R {
		self.document.with_mut(f)
	}

	/// Returns the associated file path, if any.
	pub fn path(&self) -> Option<PathBuf> {
		self.with_doc(|doc| doc.path.clone())
	}

	/// Sets the file path.
	pub fn set_path(&self, path: Option<PathBuf>) {
		self.with_doc_mut(|doc| doc.path = path);
	}

	/// Returns whether the buffer has unsaved changes.
	pub fn modified(&self) -> bool {
		self.with_doc(|doc| doc.is_modified())
	}

	/// Sets the modified flag.
	pub fn set_modified(&self, modified: bool) {
		self.with_doc_mut(|doc| doc.set_modified(modified));
	}

	/// Returns whether this buffer is read-only.
	///
	/// Checks the buffer-level override first, then falls back to the
	/// document's readonly flag.
	pub fn is_readonly(&self) -> bool {
		self.readonly_override
			.unwrap_or_else(|| self.with_doc(|doc| doc.is_readonly()))
	}

	/// Sets the read-only flag on the underlying document.
	///
	/// This affects all buffers sharing this document. For buffer-specific
	/// readonly behavior, use [`set_readonly_override`](Self::set_readonly_override).
	pub fn set_readonly(&self, readonly: bool) {
		self.with_doc_mut(|doc| doc.set_readonly(readonly));
	}

	/// Sets a buffer-level readonly override.
	///
	/// - `Some(true)`: This buffer is read-only regardless of document state
	/// - `Some(false)`: This buffer is writable regardless of document state
	/// - `None`: Defer to the document's readonly flag (default)
	///
	/// This is useful for creating read-only views (info popups, documentation
	/// panels) without affecting other buffers sharing the same document.
	pub fn set_readonly_override(&mut self, readonly: Option<bool>) {
		self.readonly_override = readonly;
	}

	/// Replaces the document content wholesale, clearing undo history.
	///
	/// This is intended for ephemeral buffers (info popups, prompts) where the
	/// entire content is replaced rather than edited incrementally. Undo history
	/// is cleared since it doesn't make sense for these use cases.
	///
	/// For normal editing operations, use the transaction-based methods instead.
	pub fn reset_content(&self, content: impl Into<xeno_primitives::Rope>) {
		self.with_doc_mut(|doc| doc.reset_content(content));
	}

	/// Returns the document version.
	pub fn version(&self) -> u64 {
		self.with_doc(|doc| doc.version())
	}

	/// Returns the file type.
	pub fn file_type(&self) -> Option<String> {
		self.with_doc(|doc| doc.file_type.clone())
	}

	/// Initializes language metadata for this buffer.
	///
	/// This populates the document's language id and file type; parsing is
	/// delegated to the syntax manager.
	pub fn init_syntax(&self, language_loader: &LanguageLoader) {
		self.with_doc_mut(|doc| doc.init_syntax(language_loader));
	}

	/// Returns the current editing mode.
	pub fn mode(&self) -> Mode {
		self.input.mode()
	}

	/// Returns a human-readable mode name.
	pub fn mode_name(&self) -> &'static str {
		self.input.mode_name()
	}

	/// Returns the line number containing the cursor.
	pub fn cursor_line(&self) -> usize {
		self.with_doc(|doc| {
			let text = doc.content();
			text.char_to_line(self.cursor.min(text.len_chars()))
		})
	}

	/// Returns the column of the cursor within its line.
	pub fn cursor_col(&self) -> usize {
		self.with_doc(|doc| {
			let text = doc.content();
			let line = text.char_to_line(self.cursor.min(text.len_chars()));
			self.cursor.saturating_sub(text.line_to_char(line))
		})
	}

	/// Computes the gutter width using the registry system.
	///
	/// This delegates to [`xeno_registry::gutter::total_width`] which computes
	/// the combined width of all enabled gutter columns.
	pub fn gutter_width(&self) -> u16 {
		use xeno_registry::gutter::{GutterWidthContext, total_width};

		self.with_doc(|doc| {
			let ctx = GutterWidthContext {
				total_lines: doc.content().len_lines(),
				viewport_width: self.text_width as u16 + 100, // approximate
			};
			total_width(&ctx)
		})
	}

	/// Returns the undo stack length.
	pub fn undo_stack_len(&self) -> usize {
		self.with_doc(|doc| doc.undo_len())
	}

	/// Returns the redo stack length.
	pub fn redo_stack_len(&self) -> usize {
		self.with_doc(|doc| doc.redo_len())
	}

	/// Clears the insert undo grouping flag.
	pub fn clear_insert_undo_active(&self) {
		self.with_doc_mut(|doc| doc.reset_insert_undo());
	}

	/// Clamps selection and cursor to valid document bounds.
	pub fn ensure_valid_selection(&mut self) {
		let max_char = self.with_doc(|doc| doc.content().len_chars());
		self.selection.clamp(max_char);
		self.cursor = self.cursor.min(max_char);
	}

	/// Asserts that selection and cursor are within valid document bounds.
	///
	/// Only active in debug builds. Use after mutations to catch invalid state early.
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

	/// No-op in release builds.
	#[cfg(not(debug_assertions))]
	#[inline]
	pub fn debug_assert_valid_state(&self) {}

	/// Maps selection and cursor through a [`Transaction`](xeno_base::Transaction).
	pub fn map_selection_through(&mut self, tx: &xeno_primitives::Transaction) {
		self.set_selection(tx.map_selection(&self.selection));
		self.sync_cursor_to_selection();
	}

	/// Resolves an option for this buffer using the layered configuration system.
	///
	/// Resolution order (highest priority first):
	/// 1. Buffer-local override (set via `:setlocal`)
	/// 2. Language-specific config (from `language "rust" { }` block)
	/// 3. Global config (from `options { }` block)
	/// 4. Compile-time default (from `#[derive_option]` macro)
	///
	/// # Example
	///
	/// ```ignore
	/// use xeno_registry::options::{keys, FromOptionValue};
	///
	/// let width = buffer.option_raw(keys::TAB_WIDTH.untyped(), editor);
	/// let tab_width = i64::from_option(&width).unwrap_or(4);
	/// ```
	pub fn option_raw(&self, key: OptionKey, editor: &crate::impls::Editor) -> OptionValue {
		editor.resolve_option(self.id, key)
	}

	/// Resolves a typed option for this buffer.
	///
	/// This is the preferred method for option access, providing compile-time
	/// type safety through [`TypedOptionKey<T>`].
	///
	/// # Example
	///
	/// ```ignore
	/// use xeno_registry::options::keys;
	///
	/// let width: i64 = buffer.option(keys::TAB_WIDTH, editor);
	/// let theme: String = buffer.option(keys::THEME, editor);
	/// ```
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
	///
	/// Use this for horizontal motion, clicks, jumps, edits - any cursor
	/// movement that should invalidate the remembered vertical column.
	#[inline]
	pub fn set_cursor(&mut self, pos: CharIdx) {
		self.cursor = pos;
		self.goal_column = None;
	}

	/// Sets selection and resets goal column.
	///
	/// Use this for horizontal motion, selections, edits - any selection
	/// change that should invalidate the remembered vertical column.
	#[inline]
	pub fn set_selection(&mut self, sel: Selection) {
		self.selection = sel;
		self.goal_column = None;
	}

	/// Syncs cursor to selection head without resetting goal column.
	///
	/// Use after selection changes when cursor should track the selection's
	/// primary head position. Does not affect goal column since the selection
	/// change already handled that.
	#[inline]
	pub fn sync_cursor_to_selection(&mut self) {
		self.cursor = self.selection.primary().head;
	}

	/// Sets both cursor and selection, resetting goal column.
	///
	/// Convenience method for the common pattern of updating both at once.
	#[inline]
	pub fn set_cursor_and_selection(&mut self, pos: CharIdx, sel: Selection) {
		self.cursor = pos;
		self.selection = sel;
		self.goal_column = None;
	}

	/// Establishes the goal column from the current cursor position.
	///
	/// The goal column is used to maintain vertical position during j/k
	/// movements through lines of varying lengths.
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
