use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::list::ListItem;
use xeno_tui::widgets::{Block, Borders, List, Widget};

use crate::editor::types::CompletionState;
#[cfg(feature = "lsp")]
use crate::editor::types::LspMenuState;
use crate::{CompletionKind, Editor};

/// Builds styled [`Span`]s for a completion label with matched characters highlighted.
///
/// Splits the `label` into segments, applying `highlight_style` to characters at
/// `match_indices` and `normal_style` elsewhere. Pads to `min_width` for alignment.
/// Returns a single padded span if no match indices are provided.
fn build_highlighted_label(
	label: &str,
	match_indices: Option<&[usize]>,
	min_width: usize,
	normal_style: Style,
	highlight_style: Style,
) -> Vec<Span<'static>> {
	let Some(indices) = match_indices else {
		return vec![Span::styled(
			format!("{:<width$}", label, width = min_width),
			normal_style,
		)];
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

	let current_len: usize = chars.len();
	if current_len < min_width {
		spans.push(Span::styled(
			" ".repeat(min_width - current_len),
			normal_style,
		));
	}

	spans
}

impl Editor {
	/// Creates a widget for rendering the completion popup menu.
	pub fn render_completion_menu(&self, _area: Rect) -> impl Widget + '_ {
		let completions = self
			.overlays
			.get::<CompletionState>()
			.cloned()
			.unwrap_or_default();

		let max_label_len = completions
			.items
			.iter()
			.map(|it| it.label.len())
			.max()
			.unwrap_or(0);

		let visible_range = completions.visible_range();
		let selected_idx = completions.selected_idx;
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
					CompletionKind::Command => self.config.theme.colors.mode.command.bg,
					CompletionKind::File => self.config.theme.colors.mode.normal.bg,
					CompletionKind::Buffer => self.config.theme.colors.semantic.accent,
					CompletionKind::Snippet => self.config.theme.colors.mode.prefix.bg,
					CompletionKind::Theme => self.config.theme.colors.semantic.accent,
				};

				let base_style = if is_selected {
					Style::default()
						.bg(self.config.theme.colors.ui.selection_bg)
						.fg(self.config.theme.colors.ui.selection_fg)
				} else {
					Style::default()
						.bg(self.config.theme.colors.popup.bg)
						.fg(self.config.theme.colors.popup.fg)
				};

				let icon_style = if is_selected {
					base_style.fg(kind_color).add_modifier(Modifier::BOLD)
				} else {
					Style::default()
						.fg(kind_color)
						.bg(self.config.theme.colors.popup.bg)
				};

				let label_style = if is_selected {
					base_style.add_modifier(Modifier::BOLD)
				} else {
					base_style
				};

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
						.fg(self.config.theme.colors.semantic.dim)
						.bg(self.config.theme.colors.popup.bg)
				};

				let match_style = label_style.fg(self.config.theme.colors.semantic.match_hl);
				let label_spans = build_highlighted_label(
					&item.label,
					item.match_indices.as_deref(),
					max_label_len,
					label_style,
					match_style,
				);

				let mut spans = vec![Span::styled(format!(" {} ", kind_icon), icon_style)];
				spans.extend(label_spans);
				spans.push(Span::styled(format!(" {:>4}  ", kind_name), dim_style));

				let line = Line::from(spans);

				ListItem::new(line).style(base_style)
			})
			.collect();

		let stripe_style = Style::default().fg(self.config.theme.colors.mode.normal.bg);
		let border_set = xeno_tui::symbols::border::Set {
			top_left: "▏",
			vertical_left: "▏",
			bottom_left: "▏",
			..xeno_tui::symbols::border::EMPTY
		};

		let block = Block::default()
			.style(Style::default().bg(self.config.theme.colors.popup.bg))
			.borders(Borders::LEFT)
			.border_set(border_set)
			.border_style(stripe_style);

		List::new(items).block(block)
	}

	/// Renders the completion popup menu if active.
	#[cfg(feature = "lsp")]
	pub fn render_completion_popup(&self, frame: &mut xeno_tui::Frame) {
		let completions = self
			.overlays
			.get::<CompletionState>()
			.cloned()
			.unwrap_or_default();
		if !completions.active || completions.items.is_empty() {
			return;
		}

		let Some(menu_state) = self.overlays.get::<LspMenuState>().and_then(|s| s.active()) else {
			return;
		};
		let buffer_id = match menu_state {
			crate::editor::types::LspMenuKind::Completion { buffer_id, .. } => *buffer_id,
			crate::editor::types::LspMenuKind::CodeAction { buffer_id, .. } => *buffer_id,
		};
		if buffer_id != self.focused_view() {
			return;
		}

		let Some(buffer) = self.get_buffer(buffer_id) else {
			return;
		};
		let tab_width = self.tab_width_for(buffer_id);
		let Some((cursor_row, cursor_col)) =
			buffer.doc_to_screen_position(buffer.cursor, tab_width)
		else {
			return;
		};

		let max_label_len = completions
			.items
			.iter()
			.map(|it| it.label.len())
			.max()
			.unwrap_or(0);
		let width = (max_label_len + 10).max(12);
		let height = completions
			.items
			.len()
			.min(CompletionState::MAX_VISIBLE)
			.max(1);

		let view_area = self.focused_view_area();
		let mut x = view_area.x.saturating_add(cursor_col);
		let mut y = view_area.y.saturating_add(cursor_row.saturating_add(1));

		let width_u16 = width.min(view_area.width as usize) as u16;
		let height_u16 = height.min(view_area.height as usize) as u16;

		if x + width_u16 > view_area.right() {
			x = view_area.right().saturating_sub(width_u16);
		}
		if y + height_u16 > view_area.bottom() {
			let above = view_area
				.y
				.saturating_add(cursor_row)
				.saturating_sub(height_u16);
			y = above.max(view_area.y);
		}

		let area = Rect::new(x, y, width_u16, height_u16);
		frame.render_widget(self.render_completion_menu(area), area);
	}
}
