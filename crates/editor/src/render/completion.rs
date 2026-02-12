use xeno_tui::layout::Rect;
use xeno_tui::style::{Color, Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::list::ListItem;
use xeno_tui::widgets::{Block, Borders, List, Widget};

use crate::{CompletionKind, CompletionState, Editor};

/// Builds styled [`Span`]s for a completion label with matched characters highlighted.
///
/// Splits the `label` into segments, applying `highlight_style` to characters at
/// `match_indices` and `normal_style` elsewhere. Pads to `min_width` for alignment.
/// Returns a single padded span if no match indices are provided.
fn build_highlighted_label(label: &str, match_indices: Option<&[usize]>, min_width: usize, normal_style: Style, highlight_style: Style) -> Vec<Span<'static>> {
	let Some(indices) = match_indices else {
		let pad = min_width.saturating_sub(crate::render::cell_width(label));
		let mut spans = vec![Span::styled(label.to_string(), normal_style)];
		if pad > 0 {
			spans.push(Span::styled(" ".repeat(pad), normal_style));
		}
		return spans;
	};

	let mut spans = Vec::new();
	let mut last_end = 0;
	let chars: Vec<char> = label.chars().collect();

	let mut sorted_indices: Vec<usize> = indices.to_vec();
	sorted_indices.sort_unstable();
	sorted_indices.dedup();

	for &idx in &sorted_indices {
		if idx >= chars.len() {
			continue;
		}
		if idx > last_end {
			let segment: String = chars[last_end..idx].iter().collect();
			spans.push(Span::styled(segment, normal_style));
		}
		spans.push(Span::styled(chars[idx].to_string(), highlight_style));
		last_end = idx + 1;
	}

	if last_end < chars.len() {
		let segment: String = chars[last_end..].iter().collect();
		spans.push(Span::styled(segment, normal_style));
	}

	let current_width: usize = chars.iter().map(|c| crate::render::char_width(*c)).sum();
	if current_width < min_width {
		spans.push(Span::styled(" ".repeat(min_width - current_width), normal_style));
	}

	spans
}

fn command_query_is_exact_alias(query: &str, label: &str) -> bool {
	let query = query.trim();
	if query.is_empty() {
		return false;
	}

	let Some(command) = xeno_registry::commands::find_command(query) else {
		return false;
	};

	!command.name_str().eq_ignore_ascii_case(query) && command.name_str().eq_ignore_ascii_case(label)
}

impl Editor {
	/// Returns whether the LSP completion popup should be rendered.
	pub fn completion_popup_visible(&self) -> bool {
		#[cfg(not(feature = "lsp"))]
		{
			false
		}

		#[cfg(feature = "lsp")]
		{
			let completions = self.overlays().get::<CompletionState>().cloned().unwrap_or_default();
			if !completions.active || completions.items.is_empty() {
				return false;
			}

			let Some(menu_state) = self.overlays().get::<crate::lsp::LspMenuState>().and_then(|state| state.active()) else {
				return false;
			};

			let buffer_id = match menu_state {
				crate::lsp::LspMenuKind::Completion { buffer_id, .. } => *buffer_id,
				crate::lsp::LspMenuKind::CodeAction { buffer_id, .. } => *buffer_id,
			};

			buffer_id == self.focused_view()
		}
	}

	/// Creates a widget for rendering the completion popup menu.
	pub fn render_completion_menu(&self, _area: Rect) -> impl Widget + '_ {
		self.render_completion_menu_with_limit(_area, CompletionState::MAX_VISIBLE)
	}

	/// Creates a widget for rendering the completion popup menu with a row limit.
	pub fn render_completion_menu_with_limit(&self, _area: Rect, max_visible_rows: usize) -> impl Widget + '_ {
		let mut completions = self.overlays().get::<CompletionState>().cloned().unwrap_or_default();
		completions.ensure_selected_visible_with_limit(max_visible_rows);

		let max_label_width = completions.items.iter().map(|it| crate::render::cell_width(&it.label)).max().unwrap_or(0);
		let show_kind = completions.show_kind && _area.width >= 24;
		let show_right = !completions.show_kind && _area.width >= 30;

		let visible_range = completions.visible_range_with_limit(max_visible_rows);
		let selected_idx = completions.selected_idx;
		let target_row_width = _area.width.saturating_sub(1) as usize;
		let items: Vec<ListItem> = completions
			.items
			.iter()
			.enumerate()
			.filter(|(i, _)| visible_range.contains(i))
			.map(|(i, item)| {
				let is_selected = Some(i) == selected_idx;

				let kind_icon = match item.kind {
					CompletionKind::Command => "󰘳",
					CompletionKind::File => "󰈔",
					CompletionKind::Buffer => "󰈙",
					CompletionKind::Snippet => "󰘦",
					CompletionKind::Theme => "󰏘",
				};

				let kind_color = match item.kind {
					CompletionKind::Command => self.state.config.theme.colors.mode.command.bg,
					CompletionKind::File => self.state.config.theme.colors.mode.normal.bg,
					CompletionKind::Buffer => self.state.config.theme.colors.semantic.accent,
					CompletionKind::Snippet => self.state.config.theme.colors.mode.prefix.bg,
					CompletionKind::Theme => self.state.config.theme.colors.semantic.accent,
				};

				let base_style = if is_selected {
					Style::default()
						.bg(self.state.config.theme.colors.ui.selection_bg)
						.fg(self.state.config.theme.colors.ui.selection_fg)
				} else {
					Style::default()
						.bg(self.state.config.theme.colors.popup.bg)
						.fg(self.state.config.theme.colors.popup.fg)
				};

				let icon_style = if is_selected {
					base_style.fg(kind_color).add_modifier(Modifier::BOLD)
				} else {
					Style::default().fg(kind_color).bg(self.state.config.theme.colors.popup.bg)
				};

				let label_style = if is_selected { base_style.add_modifier(Modifier::BOLD) } else { base_style };

				let kind_name = match item.kind {
					CompletionKind::Command => "Cmd",
					CompletionKind::File => "File",
					CompletionKind::Buffer => "Buf",
					CompletionKind::Snippet => "Snip",
					CompletionKind::Theme => "Theme",
				};

				let dim_style = if is_selected {
					base_style
				} else {
					Style::default()
						.fg(self.state.config.theme.colors.semantic.dim)
						.bg(self.state.config.theme.colors.popup.bg)
				};

				let match_color = if item.kind == CompletionKind::Command && command_query_is_exact_alias(&completions.query, &item.label) {
					Color::Magenta
				} else {
					self.state.config.theme.colors.semantic.match_hl
				};
				let match_style = label_style.fg(match_color);
				let label_spans = build_highlighted_label(&item.label, item.match_indices.as_deref(), max_label_width, label_style, match_style);

				let icon_text = format!(" {} ", kind_icon);
				let mut row_width = crate::render::cell_width(&icon_text) + max_label_width;
				let mut spans = vec![Span::styled(icon_text, icon_style)];
				spans.extend(label_spans);
				if show_kind {
					let kind_text = format!(" {:>4}  ", kind_name);
					row_width += crate::render::cell_width(&kind_text);
					spans.push(Span::styled(kind_text, dim_style));
				} else if show_right && let Some(right) = item.right.as_ref() {
					let right_width = crate::render::cell_width(right);
					if row_width + 1 + right_width <= target_row_width {
						let gap = target_row_width - row_width - right_width;
						spans.push(Span::styled(" ".repeat(gap), base_style));
						spans.push(Span::styled(right.clone(), dim_style));
						row_width = target_row_width;
					}
				}
				if row_width < target_row_width {
					spans.push(Span::styled(" ".repeat(target_row_width - row_width), base_style));
				}

				let line = Line::from(spans);

				ListItem::new(line).style(base_style)
			})
			.collect();

		let stripe_style = Style::default().fg(self.state.config.theme.colors.mode.normal.bg);
		let border_set = xeno_tui::symbols::border::Set {
			top_left: "▏",
			vertical_left: "▏",
			bottom_left: "▏",
			..xeno_tui::symbols::border::EMPTY
		};

		let block = Block::default()
			.style(Style::default().bg(self.state.config.theme.colors.popup.bg))
			.borders(Borders::LEFT)
			.border_set(border_set)
			.border_style(stripe_style);

		List::new(items).block(block)
	}

	/// Renders the completion popup menu if active.
	///
	/// Delegates to `LspSystem::render_completion_popup` which handles both
	/// the LSP-enabled and LSP-disabled cases.
	pub fn render_completion_popup(&self, frame: &mut xeno_tui::Frame) {
		self.state.lsp.render_completion_popup(self, frame);
	}
}

#[cfg(test)]
mod tests {
	use super::command_query_is_exact_alias;

	#[test]
	fn command_alias_query_detection() {
		assert!(command_query_is_exact_alias("w", "write"));
		assert!(!command_query_is_exact_alias("w", "wq"));
		assert!(!command_query_is_exact_alias("write", "write"));
	}
}
