//! Completion infrastructure.

use crate::COMMANDS;

/// Type of completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
	Command,
	File,
	Buffer,
	Snippet,
}

/// A single completion suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
	/// The text to display in the menu.
	pub label: String,
	/// The text to insert into the document.
	pub insert_text: String,
	/// Optional detail shown next to the label (e.g., command description).
	pub detail: Option<String>,
	/// Text used for filtering if different from label.
	pub filter_text: Option<String>,
	/// Kind of item.
	pub kind: CompletionKind,
}

/// Context for generating completions.
#[derive(Debug, Clone)]
pub struct CompletionContext {
	/// Current input string being completed.
	pub input: String,
	/// Cursor position within the input string.
	pub cursor: usize,
	/// The prompt character (e.g., ':', '/', etc.).
	pub prompt: char,
}

/// Provides completion items for a specific context.
pub trait CompletionSource {
	/// Generate completions for the given context.
	fn complete(&self, ctx: &CompletionContext) -> Vec<CompletionItem>;
}

/// Completion source for editor commands.
pub struct CommandSource;

impl CompletionSource for CommandSource {
	fn complete(&self, ctx: &CompletionContext) -> Vec<CompletionItem> {
		if ctx.prompt != ':' {
			return vec![];
		}

		let input = &ctx.input;

		COMMANDS
			.iter()
			.filter(|cmd| {
				cmd.name.starts_with(input) || cmd.aliases.iter().any(|a| a.starts_with(input))
			})
			.map(|cmd| CompletionItem {
				label: cmd.name.to_string(),
				insert_text: cmd.name.to_string(),
				detail: Some(cmd.description.to_string()),
				filter_text: None,
				kind: CompletionKind::Command,
			})
			.collect()
	}
}
