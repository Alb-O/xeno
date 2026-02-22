//! LSP-related type definitions.
//!
//! * [`LspMenuKind`] - Active LSP menu type (completions, code actions, references, symbols)
//! * [`LspMenuState`] - Active LSP menu state

use xeno_lsp::lsp_types::{CodeActionOrCommand, CompletionItem as LspCompletionItem};

use crate::buffer::ViewId;

/// The kind of LSP-driven menu currently active.
#[derive(Clone)]
pub enum LspMenuKind {
	Completion {
		buffer_id: ViewId,
		items: Vec<LspCompletionItem>,
	},
	CodeAction {
		buffer_id: ViewId,
		actions: Vec<CodeActionOrCommand>,
	},
	References {
		buffer_id: ViewId,
		locations: Vec<xeno_lsp::lsp_types::Location>,
		encoding: xeno_lsp::OffsetEncoding,
	},
	Symbols {
		buffer_id: ViewId,
		locations: Vec<xeno_lsp::lsp_types::Location>,
		encoding: xeno_lsp::OffsetEncoding,
	},
}

/// State for tracking the active LSP menu.
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
