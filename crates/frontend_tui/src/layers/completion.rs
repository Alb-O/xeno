use std::borrow::Cow;
use std::path::Path;

use devicons::FileIcon;
use xeno_editor::Editor;
use xeno_editor::render_api::{CompletionKind, CompletionRenderPlan};
use xeno_tui::layout::Rect;
use xeno_tui::style::{Color, Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::list::ListItem;
use xeno_tui::widgets::{Block, Borders, List};

use crate::layer::SceneBuilder;
use crate::scene::{SurfaceKind, SurfaceOp};
use crate::text_width::{cell_width, char_width};

const GENERIC_FILE_ICON: &str = "󰈔";

fn build_highlighted_label(label: &str, match_indices: Option<&[usize]>, min_width: usize, normal_style: Style, highlight_style: Style) -> Vec<Span<'static>> {
	let Some(indices) = match_indices else {
		let pad = min_width.saturating_sub(cell_width(label));
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

	let current_width: usize = chars.iter().map(|c| char_width(*c)).sum();
	if current_width < min_width {
		spans.push(Span::styled(" ".repeat(min_width - current_width), normal_style));
	}

	spans
}

fn completion_icon(kind: CompletionKind, label: &str) -> Cow<'static, str> {
	match kind {
		CompletionKind::File => {
			let icon = FileIcon::from(Path::new(label)).icon;
			if icon == '*' {
				Cow::Borrowed(GENERIC_FILE_ICON)
			} else {
				Cow::Owned(icon.to_string())
			}
		}
		CompletionKind::Command => Cow::Borrowed("󰘳"),
		CompletionKind::Buffer => Cow::Borrowed("󰈙"),
		CompletionKind::Snippet => Cow::Borrowed("󰘦"),
		CompletionKind::Theme => Cow::Borrowed("󰏘"),
	}
}

pub fn render_completion_menu(ed: &Editor, frame: &mut xeno_tui::Frame, area: Rect, plan: &CompletionRenderPlan) {
	let theme = &ed.config().theme;
	let max_label_width = plan.max_label_width();
	let show_kind = plan.show_kind();
	let show_right = plan.show_right();
	let target_row_width = plan.target_row_width();
	let items: Vec<ListItem> = plan
		.items()
		.iter()
		.map(|item| {
			let is_selected = item.selected();
			let kind_icon = completion_icon(item.kind(), item.label());

			let kind_color: Color = match item.kind() {
				CompletionKind::Command => theme.colors.mode.command.bg,
				CompletionKind::File => theme.colors.mode.normal.bg,
				CompletionKind::Buffer => theme.colors.semantic.accent,
				CompletionKind::Snippet => theme.colors.mode.prefix.bg,
				CompletionKind::Theme => theme.colors.semantic.accent,
			}
			.into();

			let base_style = if is_selected {
				Style::default().bg(theme.colors.ui.selection_bg.into()).fg(theme.colors.ui.selection_fg.into())
			} else {
				Style::default().bg(theme.colors.popup.bg.into()).fg(theme.colors.popup.fg.into())
			};

			let icon_style = if is_selected {
				base_style.fg(kind_color).add_modifier(Modifier::BOLD)
			} else {
				Style::default().fg(kind_color).bg(theme.colors.popup.bg.into())
			};

			let label_style = if is_selected { base_style.add_modifier(Modifier::BOLD) } else { base_style };

			let kind_name = match item.kind() {
				CompletionKind::Command => "Cmd",
				CompletionKind::File => "File",
				CompletionKind::Buffer => "Buf",
				CompletionKind::Snippet => "Snip",
				CompletionKind::Theme => "Theme",
			};

			let dim_style = if is_selected {
				base_style
			} else {
				Style::default().fg(theme.colors.semantic.dim.into()).bg(theme.colors.popup.bg.into())
			};

			let match_color = if item.command_alias_match() {
				Color::Magenta
			} else {
				theme.colors.semantic.match_hl.into()
			};
			let match_style = label_style.fg(match_color);
			let label_spans = build_highlighted_label(item.label(), item.match_indices(), max_label_width, label_style, match_style);

			let icon_text = format!(" {} ", kind_icon);
			let mut row_width = cell_width(&icon_text) + max_label_width;
			let mut spans = vec![Span::styled(icon_text, icon_style)];
			spans.extend(label_spans);
			if show_kind {
				let kind_text = format!(" {:>4}  ", kind_name);
				row_width += cell_width(&kind_text);
				spans.push(Span::styled(kind_text, dim_style));
			} else if show_right && let Some(right) = item.right() {
				let right_width = cell_width(right);
				if row_width + 1 + right_width <= target_row_width {
					let gap = target_row_width - row_width - right_width;
					spans.push(Span::styled(" ".repeat(gap), base_style));
					spans.push(Span::styled(right.to_string(), dim_style));
					row_width = target_row_width;
				}
			}
			if row_width < target_row_width {
				spans.push(Span::styled(" ".repeat(target_row_width - row_width), base_style));
			}

			ListItem::new(Line::from(spans)).style(base_style)
		})
		.collect();

	let stripe_style = Style::default().fg(theme.colors.mode.normal.bg.into());
	let border_set = xeno_tui::symbols::border::Set {
		top_left: "▏",
		vertical_left: "▏",
		bottom_left: "▏",
		..xeno_tui::symbols::border::EMPTY
	};

	let block = Block::default()
		.style(Style::default().bg(theme.colors.popup.bg.into()))
		.borders(Borders::LEFT)
		.border_set(border_set)
		.border_style(stripe_style);
	frame.render_widget(List::new(items).block(block), area);
}

pub fn visible(ed: &Editor) -> bool {
	ed.completion_popup_visible()
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(SurfaceKind::CompletionPopup, 40, doc_area, SurfaceOp::CompletionPopup, false);
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame) {
	let Some(area) = ed.completion_popup_area() else {
		return;
	};
	let Some(plan) = ed.completion_popup_render_plan() else {
		return;
	};
	render_completion_menu(ed, frame, area.into(), &plan);
}
