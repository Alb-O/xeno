//! LSP-driven menu state (completions, code actions).

use xeno_lsp::lsp_types::{CodeActionOrCommand, CompletionItem};

use crate::buffer::BufferId;

#[derive(Clone)]
pub enum LspMenuKind {
	Completion {
		buffer_id: BufferId,
		items: Vec<CompletionItem>,
	},
	CodeAction {
		buffer_id: BufferId,
		actions: Vec<CodeActionOrCommand>,
	},
}

#[derive(Clone, Default)]
pub struct LspMenuState {
	kind: Option<LspMenuKind>,
}

impl LspMenuState {
	pub fn set(&mut self, kind: LspMenuKind) {
		self.kind = Some(kind);
	}

	pub fn clear(&mut self) {
		self.kind = None;
	}

	pub fn active(&self) -> Option<&LspMenuKind> {
		self.kind.as_ref()
	}

	pub fn is_active(&self) -> bool {
		self.kind.is_some()
	}
}
