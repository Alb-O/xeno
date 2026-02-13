use iced::widget::text::Wrapping;
use iced::widget::{Column, column, row, text};
use iced::{Color, Font};
use xeno_editor::render_api::{
	CompletionKind, CompletionRenderItem, CompletionRenderPlan, InfoPopupRenderAnchor, Rect, SnippetChoiceRenderItem, SnippetChoiceRenderPlan,
};

use super::Message;
use crate::runtime::SurfaceSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorTone {
	Normal,
	Meta,
	Selected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletionRowParts {
	marker: &'static str,
	label: String,
	kind: Option<String>,
	right: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnippetRowParts {
	marker: &'static str,
	option: String,
}

pub(super) fn render_inspector_rows(surface: &SurfaceSnapshot) -> Column<'static, Message> {
	let mut rows = column![].spacing(2);
	rows = push_inspector_section_title(rows, "surface");
	rows = append_surface_rows(rows, surface);
	rows = rows.push(styled_inspector_text(String::new(), InspectorTone::Normal));
	rows = push_inspector_section_title(rows, "completion");
	rows = append_completion_rows(rows, surface.completion_plan.as_ref());
	rows = rows.push(styled_inspector_text(String::new(), InspectorTone::Normal));
	rows = push_inspector_section_title(rows, "snippet");
	append_snippet_rows(rows, surface.snippet_plan.as_ref())
}

fn push_inspector_section_title(mut rows: Column<'static, Message>, title: &str) -> Column<'static, Message> {
	rows = rows.push(styled_inspector_text(format!("{title}:"), InspectorTone::Normal));
	rows
}

fn styled_inspector_text(content: impl Into<String>, tone: InspectorTone) -> iced::widget::Text<'static> {
	let mut row_text = text(content.into()).font(Font::MONOSPACE).wrapping(Wrapping::None);
	row_text = match tone {
		InspectorTone::Normal => row_text,
		InspectorTone::Meta => row_text.color(Color::from_rgb8(0x6A, 0x73, 0x7D)),
		InspectorTone::Selected => row_text.color(Color::from_rgb8(0x0B, 0x72, 0x2B)),
	};
	row_text
}

fn append_surface_rows(mut rows: Column<'static, Message>, surface: &SurfaceSnapshot) -> Column<'static, Message> {
	match surface.overlay_kind {
		Some(kind) => {
			rows = rows.push(styled_inspector_text(
				format!("overlay={kind:?} panes={}", surface.overlay_panes.len()),
				InspectorTone::Meta,
			));
			for pane in surface.overlay_panes.iter().take(3) {
				rows = rows.push(styled_inspector_text(
					format!("  {:?} {}", pane.role(), rect_brief(pane.rect())),
					InspectorTone::Meta,
				));
			}
			if surface.overlay_panes.len() > 3 {
				rows = rows.push(styled_inspector_text(
					format!("  ... {} more panes", surface.overlay_panes.len() - 3),
					InspectorTone::Meta,
				));
			}
		}
		None => {
			rows = rows.push(styled_inspector_text("overlay=none", InspectorTone::Meta));
		}
	}

	match surface.completion_plan.as_ref() {
		Some(plan) => {
			let selected = plan
				.items()
				.iter()
				.find(|item| item.selected())
				.map_or_else(|| String::from("-"), |item| item.label().to_string());
			rows = rows.push(styled_inspector_text(
				format!(
					"completion=visible rows={} selected={} kind_col={} right_col={}",
					plan.items().len(),
					selected,
					plan.show_kind(),
					plan.show_right()
				),
				InspectorTone::Meta,
			));
		}
		None => {
			rows = rows.push(styled_inspector_text("completion=hidden", InspectorTone::Meta));
		}
	}

	match surface.snippet_plan.as_ref() {
		Some(plan) => {
			let selected = plan
				.items()
				.iter()
				.find(|item| item.selected())
				.map_or_else(|| String::from("-"), |item| item.option().to_string());
			rows = rows.push(styled_inspector_text(
				format!("snippet_choice=visible rows={} selected={selected}", plan.items().len()),
				InspectorTone::Meta,
			));
		}
		None => {
			rows = rows.push(styled_inspector_text("snippet_choice=hidden", InspectorTone::Meta));
		}
	}

	if surface.info_popup_plan.is_empty() {
		rows = rows.push(styled_inspector_text("info_popups=none", InspectorTone::Meta));
	} else {
		rows = rows.push(styled_inspector_text(
			format!("info_popups={}", surface.info_popup_plan.len()),
			InspectorTone::Meta,
		));
		for popup in surface.info_popup_plan.iter().take(2) {
			let anchor = match popup.anchor() {
				InfoPopupRenderAnchor::Center => String::from("center"),
				InfoPopupRenderAnchor::Point { x, y } => format!("point@{x},{y}"),
				InfoPopupRenderAnchor::Window(wid) => format!("window@{wid:?}"),
			};
			rows = rows.push(styled_inspector_text(
				format!(
					"  popup#{} {} {}x{}",
					popup.id().as_u64(),
					anchor,
					popup.content_width(),
					popup.content_height()
				),
				InspectorTone::Meta,
			));
		}
		if surface.info_popup_plan.len() > 2 {
			rows = rows.push(styled_inspector_text(
				format!("  ... {} more popups", surface.info_popup_plan.len() - 2),
				InspectorTone::Meta,
			));
		}
	}

	rows
}

fn append_completion_rows(mut rows: Column<'static, Message>, plan: Option<&CompletionRenderPlan>) -> Column<'static, Message> {
	let Some(plan) = plan else {
		rows = rows.push(styled_inspector_text("completion_rows=hidden", InspectorTone::Meta));
		return rows;
	};

	rows = rows.push(styled_inspector_text(
		format!(
			"completion_rows={} target_width={} kind_col={} right_col={}",
			plan.items().len(),
			plan.target_row_width(),
			plan.show_kind(),
			plan.show_right()
		),
		InspectorTone::Meta,
	));

	for item in plan.items().iter().take(8) {
		let tone = if item.selected() { InspectorTone::Selected } else { InspectorTone::Normal };
		rows = rows.push(render_completion_row(plan, item, tone));
	}

	if plan.items().len() > 8 {
		rows = rows.push(styled_inspector_text(
			format!("... {} more completion rows", plan.items().len() - 8),
			InspectorTone::Meta,
		));
	}

	rows
}

fn completion_row_parts(plan: &CompletionRenderPlan, item: &CompletionRenderItem) -> CompletionRowParts {
	CompletionRowParts {
		marker: if item.selected() { ">" } else { " " },
		label: item.label().to_string(),
		kind: if plan.show_kind() { Some(format!("{:?}", item.kind())) } else { None },
		right: if plan.show_right() { item.right().map(str::to_string) } else { None },
	}
}

fn render_completion_row(plan: &CompletionRenderPlan, item: &CompletionRenderItem, tone: InspectorTone) -> iced::widget::Row<'static, Message> {
	let parts = completion_row_parts(plan, item);
	let mut row_widget = row![styled_inspector_text(parts.marker, tone), styled_inspector_text(parts.label, tone),].spacing(1);

	if let Some(kind) = parts.kind {
		row_widget = row_widget.push(styled_inspector_text(format!("[{kind}]"), tone));
	}
	if let Some(right) = parts.right {
		row_widget = row_widget.push(styled_inspector_text(format!("({right})"), tone));
	}

	row_widget
}

fn append_snippet_rows(mut rows: Column<'static, Message>, plan: Option<&SnippetChoiceRenderPlan>) -> Column<'static, Message> {
	let Some(plan) = plan else {
		rows = rows.push(styled_inspector_text("snippet_rows=hidden", InspectorTone::Meta));
		return rows;
	};

	rows = rows.push(styled_inspector_text(
		format!("snippet_rows={} target_width={}", plan.items().len(), plan.target_row_width()),
		InspectorTone::Meta,
	));

	for item in plan.items().iter().take(8) {
		let tone = if item.selected() { InspectorTone::Selected } else { InspectorTone::Normal };
		rows = rows.push(render_snippet_row(item, tone));
	}

	if plan.items().len() > 8 {
		rows = rows.push(styled_inspector_text(
			format!("... {} more snippet rows", plan.items().len() - 8),
			InspectorTone::Meta,
		));
	}

	rows
}

fn snippet_row_parts(item: &SnippetChoiceRenderItem) -> SnippetRowParts {
	SnippetRowParts {
		marker: if item.selected() { ">" } else { " " },
		option: item.option().to_string(),
	}
}

fn render_snippet_row(item: &SnippetChoiceRenderItem, tone: InspectorTone) -> iced::widget::Row<'static, Message> {
	let parts = snippet_row_parts(item);
	row![styled_inspector_text(parts.marker, tone), styled_inspector_text(parts.option, tone),].spacing(1)
}

fn rect_brief(rect: Rect) -> String {
	format!("{}x{}@{},{}", rect.width, rect.height, rect.x, rect.y)
}

#[cfg(test)]
mod tests;
