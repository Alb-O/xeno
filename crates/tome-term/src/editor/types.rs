use tome_core::registry::CompletionItem;
use tome_core::{Rope, Selection};

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

#[derive(Clone, Default)]
pub struct CompletionState {
	pub items: Vec<CompletionItem>,
	pub selected_idx: Option<usize>,
	pub active: bool,
}
