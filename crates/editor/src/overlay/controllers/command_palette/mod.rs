//! Command palette overlay controller with command and path completion.

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use xeno_primitives::{Key, KeyCode, Selection};
use xeno_registry::commands::{COMMANDS, PaletteArgKind, PaletteCommitPolicy};
use xeno_registry::notifications::keys;
use xeno_registry::options::{OPTIONS, OptionType, OptionValue, option_keys as opt_keys};
use xeno_registry::snippets::SNIPPETS;
use xeno_registry::themes::{THEMES, ThemeVariant};

use crate::completion::{CompletionFileMeta, CompletionItem, CompletionKind, CompletionState, SelectionIntent};
use crate::overlay::picker_engine::model::{CommitDecision, PickerAction};
use crate::overlay::picker_engine::providers::{FnPickerProvider, PickerProvider};
use crate::overlay::{CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy};
use crate::window::GutterSelector;

#[derive(Debug, Clone)]
struct TokenCtx {
	cmd: String,
	token_index: usize,
	start: usize,
	query: String,
	args: Vec<String>,
	path_dir: Option<String>,
	quoted: Option<char>,
	close_quote_idx: Option<usize>,
}

type Tok = crate::overlay::picker_engine::parser::PickerToken;

pub struct CommandPaletteOverlay {
	last_input: String,
	selected_label: Option<String>,
	last_token_index: Option<usize>,
	file_cache: Option<(PathBuf, Vec<(String, bool)>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandArgCompletion {
	None,
	FilePath,
	Snippet,
	Theme,
	OptionKey,
	OptionValue,
	Buffer,
	CommandName,
	FreeText,
}

impl CommandArgCompletion {
	fn from_palette_kind(kind: PaletteArgKind) -> Self {
		match kind {
			PaletteArgKind::FilePath => Self::FilePath,
			PaletteArgKind::ThemeName => Self::Theme,
			PaletteArgKind::SnippetRefOrBody => Self::Snippet,
			PaletteArgKind::OptionKey => Self::OptionKey,
			PaletteArgKind::OptionValue => Self::OptionValue,
			PaletteArgKind::BufferRef => Self::Buffer,
			PaletteArgKind::CommandName => Self::CommandName,
			PaletteArgKind::FreeText => Self::FreeText,
		}
	}

	fn completion_kind(self) -> Option<CompletionKind> {
		match self {
			Self::None | Self::FreeText => None,
			Self::FilePath => Some(CompletionKind::File),
			Self::Snippet => Some(CompletionKind::Snippet),
			Self::Theme => Some(CompletionKind::Theme),
			Self::OptionKey | Self::OptionValue | Self::CommandName => Some(CompletionKind::Command),
			Self::Buffer => Some(CompletionKind::Buffer),
		}
	}

	fn supports_completion(self) -> bool {
		self.completion_kind().is_some()
	}
}

mod apply;
mod commit;
mod controller;
mod parser;
mod providers;
mod selection;

#[cfg(test)]
mod tests;
