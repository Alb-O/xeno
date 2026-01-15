//! Buffer - the core text editing unit.
//!
//! The buffer system separates document content from view state:
//! - [`Document`] holds shared content (text, undo history, syntax)
//! - [`Buffer`] holds per-view state (cursor, selection, scroll position)
//!
//! Multiple buffers can share the same document, enabling proper split behavior.

mod document;
mod editing;
mod history;
mod layout;
mod navigation;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub use document::{Document, DocumentId};
pub use history::HistoryResult;
pub use layout::{BufferView, Layout, SpatialDirection, SplitDirection, SplitPath};
use xeno_primitives::range::CharIdx;
use xeno_primitives::{Mode, Selection};
use xeno_input::InputHandler;
use xeno_runtime_language::LanguageLoader;
use xeno_registry::options::{
	FromOptionValue, OptionKey, OptionStore, OptionValue, TypedOptionKey,
};

/// Unique identifier for a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(pub u64);

impl BufferId {
	/// Identifier for the default scratch buffer.
	pub const SCRATCH: BufferId = BufferId(0);
}

/// A text buffer - combines a view with its document.
///
/// Buffer is now a wrapper that provides convenient access to both view state
/// (cursor, selection, scroll) and document state (content, undo history, syntax).
///
/// For split views, multiple Buffers can share the same underlying Document.
pub struct Buffer {
	/// Unique identifier for this buffer/view.
	pub id: BufferId,

	/// The underlying document (shared across split views).
	document: Arc<RwLock<Document>>,

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

	/// Whether to suppress auto-scroll down to keep the cursor visible.
	pub suppress_scroll_down: bool,

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
	pub fn new(id: BufferId, content: String, path: Option<PathBuf>) -> Self {
		let document = Arc::new(RwLock::new(Document::new(content, path)));
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
			suppress_scroll_down: false,
			local_options: OptionStore::new(),
			readonly_override: None,
			goal_column: None,
		}
	}

	/// Creates a new scratch buffer.
	pub fn scratch(id: BufferId) -> Self {
		Self::new(id, String::new(), None)
	}

	/// Creates a new buffer that shares the same document (for split views).
	///
	/// The new buffer has independent cursor/selection/scroll state but
	/// edits in either buffer affect both. Local options are cloned so each
	/// split can have independent option overrides. The readonly override is
	/// intentionally NOT cloned - splits start with no override (deferring to
	/// the document's readonly state).
	pub fn clone_for_split(&self, new_id: BufferId) -> Self {
		Self {
			id: new_id,
			document: Arc::clone(&self.document),
			cursor: self.cursor,
			selection: self.selection.clone(),
			input: InputHandler::new(),
			scroll_line: self.scroll_line,
			scroll_segment: self.scroll_segment,
			text_width: self.text_width,
			last_viewport_height: 0,
			last_rendered_cursor: self.cursor,
			suppress_scroll_down: false,
			local_options: self.local_options.clone(),
			readonly_override: None,
			goal_column: None,
		}
	}

	/// Returns the document ID.
	pub fn document_id(&self) -> DocumentId {
		self.document.read().unwrap().id
	}

	/// Returns a clone of the document Arc (for creating split views).
	pub fn document_arc(&self) -> Arc<RwLock<Document>> {
		Arc::clone(&self.document)
	}

	/// Checks if this buffer shares a document with another buffer.
	pub fn shares_document_with(&self, other: &Buffer) -> bool {
		Arc::ptr_eq(&self.document, &other.document)
	}

	/// Returns the document content (read-only borrow of the Rope).
	///
	/// For mutation, use the editing methods which handle locking properly.
	#[inline]
	pub fn doc(&self) -> std::sync::RwLockReadGuard<'_, Document> {
		self.document.read().unwrap()
	}

	/// Returns mutable access to the document.
	#[inline]
	pub fn doc_mut(&self) -> std::sync::RwLockWriteGuard<'_, Document> {
		self.document.write().unwrap()
	}

	/// Returns the associated file path.
	pub fn path(&self) -> Option<PathBuf> {
		self.document.read().unwrap().path.clone()
	}

	/// Sets the file path.
	pub fn set_path(&self, path: Option<PathBuf>) {
		self.document.write().unwrap().path = path;
	}

	/// Returns whether the buffer has unsaved changes.
	pub fn modified(&self) -> bool {
		self.document.read().unwrap().modified
	}

	/// Sets the modified flag.
	pub fn set_modified(&self, modified: bool) {
		self.document.write().unwrap().modified = modified;
	}

	/// Returns whether this buffer is read-only.
	///
	/// Checks the buffer-level override first, then falls back to the
	/// document's readonly flag.
	pub fn is_readonly(&self) -> bool {
		self.readonly_override
			.unwrap_or_else(|| self.document.read().unwrap().readonly)
	}

	/// Sets the read-only flag on the underlying document.
	///
	/// This affects all buffers sharing this document. For buffer-specific
	/// readonly behavior, use [`set_readonly_override`](Self::set_readonly_override).
	pub fn set_readonly(&self, readonly: bool) {
		self.document.write().unwrap().readonly = readonly;
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

	/// Returns the document version.
	pub fn version(&self) -> u64 {
		self.document.read().unwrap().version
	}

	/// Returns the file type.
	pub fn file_type(&self) -> Option<String> {
		self.document.read().unwrap().file_type.clone()
	}

	/// Returns whether syntax highlighting is available.
	pub fn has_syntax(&self) -> bool {
		self.document.read().unwrap().syntax.is_some()
	}

	/// Initializes syntax highlighting for this buffer.
	pub fn init_syntax(&self, language_loader: &LanguageLoader) {
		self.document.write().unwrap().init_syntax(language_loader);
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
		let doc = self.document.read().unwrap();
		let max_pos = doc.content.len_chars();
		doc.content.char_to_line(self.cursor.min(max_pos))
	}

	/// Returns the column of the cursor within its line.
	pub fn cursor_col(&self) -> usize {
		let doc = self.document.read().unwrap();
		let line = doc
			.content
			.char_to_line(self.cursor.min(doc.content.len_chars()));
		let line_start = doc.content.line_to_char(line);
		self.cursor.saturating_sub(line_start)
	}

	/// Computes the gutter width using the registry system.
	///
	/// This delegates to [`xeno_registry::gutter::total_width`] which computes
	/// the combined width of all enabled gutter columns.
	pub fn gutter_width(&self) -> u16 {
		use xeno_registry::gutter::{GutterWidthContext, total_width};

		let doc = self.document.read().unwrap();
		let ctx = GutterWidthContext {
			total_lines: doc.content.len_lines(),
			viewport_width: self.text_width as u16 + 100, // approximate
		};
		total_width(&ctx)
	}

	/// Reparses the entire syntax tree from scratch.
	pub fn reparse_syntax(&self, language_loader: &LanguageLoader) {
		self.document
			.write()
			.unwrap()
			.reparse_syntax(language_loader);
	}

	/// Returns the undo stack length.
	pub fn undo_stack_len(&self) -> usize {
		self.document.read().unwrap().undo_stack.len()
	}

	/// Returns the redo stack length.
	pub fn redo_stack_len(&self) -> usize {
		self.document.read().unwrap().redo_stack.len()
	}

	/// Clears the insert undo grouping flag.
	pub fn clear_insert_undo_active(&self) {
		self.doc_mut().insert_undo_active = false;
	}

	/// Clamps selection and cursor to valid document bounds.
	pub fn ensure_valid_selection(&mut self) {
		let max_char = self.doc().content.len_chars();
		self.selection.clamp(max_char);
		self.cursor = self.cursor.min(max_char);
	}

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
	pub fn option_raw(&self, key: OptionKey, editor: &crate::editor::Editor) -> OptionValue {
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
		editor: &crate::editor::Editor,
	) -> T {
		T::from_option(&self.option_raw(key.untyped(), editor))
			.or_else(|| T::from_option(&(key.def().default)()))
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

	/// Establishes goal column from current cursor position.
	///
	/// Use after explicit horizontal positioning (mouse click) to set the
	/// goal column for subsequent vertical navigation.
	#[inline]
	pub fn establish_goal_column(&mut self) {
		let col = {
			let doc = self.doc();
			let line = doc
				.content
				.char_to_line(self.cursor.min(doc.content.len_chars()));
			let line_start = doc.content.line_to_char(line);
			self.cursor.saturating_sub(line_start)
		};
		self.goal_column = Some(col);
	}
}
