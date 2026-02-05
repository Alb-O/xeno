//! Theme completion source.

use xeno_registry::themes::{THEMES, ThemeVariant};

use crate::completion::{
	CompletionContext, CompletionItem, CompletionKind, CompletionResult, CompletionSource,
	PROMPT_COMMAND,
};

/// Completion source for theme names.
pub struct ThemeSource;

impl CompletionSource for ThemeSource {
	fn complete(&self, ctx: &CompletionContext) -> CompletionResult {
		if ctx.prompt != PROMPT_COMMAND {
			return CompletionResult::empty();
		}

		let parts: Vec<&str> = ctx.input.split_whitespace().collect();
		if !matches!(parts.first(), Some(&"theme") | Some(&"colorscheme")) {
			return CompletionResult::empty();
		}

		let prefix = parts.get(1).copied().unwrap_or("");
		if parts.len() == 1 && !ctx.input.ends_with(' ') {
			return CompletionResult::empty();
		}

		let cmd_name = parts.first().unwrap();
		let arg_start = cmd_name.len() + 1;

		let mut items: Vec<_> = THEMES
			.all()
			.iter()
			.map(|t| &**t)
			.filter(|t| {
				t.meta.name.starts_with(prefix)
					|| t.meta.aliases.iter().any(|a| a.starts_with(prefix))
			})
			.map(|t| CompletionItem {
				label: t.meta.name.to_string(),
				insert_text: t.meta.name.to_string(),
				detail: Some(format!(
					"{} theme",
					match t.variant {
						ThemeVariant::Dark => "dark",
						ThemeVariant::Light => "light",
					}
				)),
				filter_text: None,
				kind: CompletionKind::Theme,
				match_indices: None,
			})
			.collect();

		items.dedup_by(|a, b| a.label == b.label);
		CompletionResult::new(arg_start, items)
	}
}
