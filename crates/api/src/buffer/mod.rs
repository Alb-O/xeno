//! Buffer - the core text editing unit.
//!
//! A `Buffer` represents a single text document with its associated state:
//! - Text content (Rope)
//! - Cursor and selection (multi-cursor support)
//! - Input handling (modal state)
//! - File association and modification tracking
//! - Undo/redo history
//! - Scroll state
//! - Syntax highlighting

mod editing;
mod history;
mod layout;
mod navigation;

use std::path::PathBuf;

pub use history::HistoryResult;
pub use layout::{Layout, SplitDirection};
use tome_base::range::CharIdx;
use tome_base::{Rope, Selection};
use tome_input::InputHandler;
use tome_language::LanguageLoader;
use tome_language::syntax::Syntax;
use tome_manifest::Mode;

use crate::editor::types::HistoryEntry;

/// Unique identifier for a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(pub u64);

impl BufferId {
	pub const SCRATCH: BufferId = BufferId(0);
}

/// A text buffer - the core editing unit.
///
/// Each buffer represents one file or scratch document. It contains all
/// state needed for editing: text content, cursor/selection, input mode,
/// undo history, and syntax highlighting.
///
/// Buffers are managed by a `Workspace` which provides shared resources
/// like themes, registers, and filesystem access.
pub struct Buffer {
	/// Unique identifier for this buffer.
	pub id: BufferId,

	/// The document content.
	pub doc: Rope,

	/// Primary cursor position (char index).
	/// This is the head of the primary range in the selection.
	pub cursor: CharIdx,

	/// Multi-cursor selection state.
	pub selection: Selection,

	/// Modal input handler (tracks mode, pending keys, count).
	pub input: InputHandler,

	/// Associated file path (None for scratch buffers).
	pub path: Option<PathBuf>,

	/// Whether the buffer has unsaved changes.
	pub modified: bool,

	/// Undo history stack.
	pub undo_stack: Vec<HistoryEntry>,

	/// Redo history stack.
	pub redo_stack: Vec<HistoryEntry>,

	/// Scroll position: first visible line.
	pub scroll_line: usize,

	/// Scroll position: first visible segment within the line (for wrapped lines).
	pub scroll_segment: usize,

	/// Text width for wrapping calculations.
	pub text_width: usize,

	/// Detected file type (e.g., "rust", "python").
	pub file_type: Option<String>,

	/// Syntax highlighting state.
	pub syntax: Option<Syntax>,

	/// Flag for grouping insert-mode edits into a single undo.
	pub(crate) insert_undo_active: bool,
}

impl Buffer {
	/// Creates a new buffer with the given ID and content.
	pub fn new(id: BufferId, content: String, path: Option<PathBuf>) -> Self {
		let doc = Rope::from(content.as_str());
		Self {
			id,
			doc,
			cursor: 0,
			selection: Selection::point(0),
			input: InputHandler::new(),
			path,
			modified: false,
			undo_stack: Vec::new(),
			redo_stack: Vec::new(),
			scroll_line: 0,
			scroll_segment: 0,
			text_width: 80,
			file_type: None,
			syntax: None,
			insert_undo_active: false,
		}
	}

	/// Creates a new scratch buffer.
	pub fn scratch(id: BufferId) -> Self {
		Self::new(id, String::new(), None)
	}

	/// Initializes syntax highlighting for this buffer.
	pub fn init_syntax(&mut self, language_loader: &LanguageLoader) {
		if let Some(ref p) = self.path
			&& let Some(lang_id) = language_loader.language_for_path(p) {
				let lang_data = language_loader.get(lang_id);
				self.file_type = lang_data.map(|l| l.name.clone());
				self.syntax = Syntax::new(self.doc.slice(..), lang_id, language_loader).ok();
			}
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
		let max_pos = self.doc.len_chars();
		self.doc.char_to_line(self.cursor.min(max_pos))
	}

	/// Returns the column of the cursor within its line.
	pub fn cursor_col(&self) -> usize {
		let line = self.cursor_line();
		let line_start = self.doc.line_to_char(line);
		self.cursor.saturating_sub(line_start)
	}

	/// Minimum gutter width padding.
	const GUTTER_MIN_WIDTH: u16 = 4;

	/// Computes the gutter width based on total line count.
	pub fn gutter_width(&self) -> u16 {
		let total_lines = self.doc.len_lines();
		(total_lines.max(1).ilog10() as u16 + 2).max(Self::GUTTER_MIN_WIDTH)
	}

	/// Reparses the entire syntax tree from scratch.
	///
	/// Used after operations that replace the entire document (undo/redo).
	pub fn reparse_syntax(&mut self, language_loader: &LanguageLoader) {
		if self.syntax.is_some() {
			let lang_id = self.syntax.as_ref().unwrap().root_language();
			self.syntax = Syntax::new(self.doc.slice(..), lang_id, language_loader).ok();
		}
	}
}
