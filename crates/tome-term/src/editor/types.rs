use std::path::PathBuf;

use tome_core::{InputHandler, Rope, Selection};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MessageKind {
	Info,
	Error,
}

#[derive(Clone, Debug)]
pub struct Message {
	pub text: String,
	pub kind: MessageKind,
}

/// A history entry for undo/redo.
#[derive(Clone)]
pub struct HistoryEntry {
	pub doc: Rope,
	pub selection: Selection,
}

#[derive(Default)]
pub struct Registers {
	pub yank: String,
}

#[derive(Clone)]
pub struct ScratchState {
	pub doc: Rope,
	pub cursor: usize,
	pub selection: Selection,
	pub input: InputHandler,
	pub path: Option<PathBuf>,
	pub modified: bool,
	pub scroll_line: usize,
	pub scroll_segment: usize,
	pub undo_stack: Vec<HistoryEntry>,
	pub redo_stack: Vec<HistoryEntry>,
	pub text_width: usize,
	pub insert_undo_active: bool,
}

impl Default for ScratchState {
	fn default() -> Self {
		Self {
			doc: Rope::from(""),
			cursor: 0,
			selection: Selection::point(0),
			input: InputHandler::new(),
			path: None,
			modified: false,
			scroll_line: 0,
			scroll_segment: 0,
			undo_stack: Vec::new(),
			redo_stack: Vec::new(),
			text_width: 80,
			insert_undo_active: false,
		}
	}
}
