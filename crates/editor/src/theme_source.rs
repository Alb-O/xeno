//! Theme completion source.

use xeno_registry::themes::{THEMES, ThemeVariant};

use crate::completion::{CompletionContext, CompletionItem, CompletionKind, CompletionResult, CompletionSource, PROMPT_COMMAND};

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

		let mut scored: Vec<(i32, CompletionItem)> = THEMES
			.snapshot_guard()
			.iter_refs()
			.filter_map(|t| {
				let name = t.name_str();
				let mut best = i32::MIN;
				let mut match_indices = None;

				if let Some((score, _, indices)) = crate::completion::frizbee_match(prefix, name) {
					best = score as i32 + 200;
					if !indices.is_empty() {
						match_indices = Some(indices);
					}
				}
				for alias in t.keys_resolved() {
					if let Some((score, _, _)) = crate::completion::frizbee_match(prefix, alias) {
						best = best.max(score as i32 + 80);
					}
				}
				if prefix.is_empty() {
					best = 0;
				}
				if !prefix.is_empty() && best == i32::MIN {
					return None;
				}

				let variant = match t.variant {
					ThemeVariant::Dark => "dark",
					ThemeVariant::Light => "light",
				};

				Some((
					best,
					CompletionItem {
						label: name.to_string(),
						insert_text: name.to_string(),
						detail: Some(format!("{variant} theme")),
						filter_text: None,
						kind: CompletionKind::Theme,
						match_indices,
						right: Some(variant.to_string()),
					},
				))
			})
			.collect();

		scored.sort_by(|(score_a, item_a), (score_b, item_b)| score_b.cmp(score_a).then_with(|| item_a.label.cmp(&item_b.label)));

		let mut items: Vec<CompletionItem> = Vec::new();
		for (_, item) in scored {
			if items.iter().any(|existing| existing.label == item.label) {
				continue;
			}
			items.push(item);
		}
		CompletionResult::new(arg_start, items)
	}
}
