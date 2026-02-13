use xeno_editor::completion::CompletionRenderPlan;
use xeno_editor::geometry::Rect;
use xeno_editor::info_popup::{InfoPopupRenderAnchor, InfoPopupRenderTarget};
use xeno_editor::overlay::{OverlayControllerKind, OverlayPaneRenderTarget};
use xeno_editor::render_api::{BufferRenderContext, RenderLine, RenderSpan};
use xeno_editor::snippet::SnippetChoiceRenderPlan;
use xeno_editor::{Buffer, Editor, ViewId};
use xeno_primitives::Style;

const MAX_VISIBLE_BUFFER_LINES: usize = 500;

#[derive(Debug, Default)]
pub(crate) struct Snapshot {
	pub(crate) title: String,
	pub(crate) header: String,
	pub(crate) statusline: String,
	pub(crate) document_lines: Vec<RenderLine<'static>>,
	pub(crate) inspector_sections: Vec<InspectorSection>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct InspectorSection {
	pub(crate) title: String,
	pub(crate) rows: Vec<String>,
}

impl InspectorSection {
	fn new(title: &str, mut rows: Vec<String>) -> Self {
		if rows.is_empty() {
			rows.push(String::from("-"));
		}

		Self {
			title: title.to_string(),
			rows,
		}
	}
}

pub(crate) fn build_snapshot(editor: &mut Editor, ime_preedit: Option<&str>) -> Snapshot {
	editor.ensure_syntax_for_buffers();

	let mode = editor.mode_name();
	let cursor_line = editor.cursor_line() + 1;
	let cursor_col = editor.cursor_col() + 1;
	let buffers = editor.buffer_count();
	let focused = editor.focused_view();
	let statusline = editor
		.statusline_render_plan()
		.into_iter()
		.map(|segment| segment.text)
		.collect::<Vec<_>>()
		.join("");
	let overlay_kind = editor.overlay_kind();
	let overlay_panes = editor.overlay_pane_render_plan();
	let completion_plan = editor.completion_popup_render_plan();
	let snippet_plan = editor.snippet_choice_render_plan();
	let info_popup_plan = editor.info_popup_render_plan();

	let inspector_sections = vec![
		InspectorSection::new(
			"surface",
			build_surface_summary_rows(overlay_kind, &overlay_panes, completion_plan.as_ref(), snippet_plan.as_ref(), &info_popup_plan),
		),
		InspectorSection::new("completion", build_completion_preview_rows(completion_plan.as_ref())),
		InspectorSection::new("snippet", build_snippet_preview_rows(snippet_plan.as_ref())),
	];

	let (title, document_lines) = snapshot_for_focused_view(editor, focused).unwrap_or_else(|| {
		editor.get_buffer(focused).map_or_else(
			|| (String::from("xeno-iced"), vec![plain_line("no focused buffer")]),
			|buffer| snapshot_for_buffer(buffer),
		)
	});

	Snapshot {
		title,
		header: format!(
			"mode={mode} cursor={cursor_line}:{cursor_col} buffers={buffers} ime_preedit={}",
			ime_preedit_label(ime_preedit)
		),
		statusline: compact_statusline(statusline),
		document_lines,
		inspector_sections,
	}
}

fn snapshot_for_buffer(buffer: &Buffer) -> (String, Vec<RenderLine<'static>>) {
	let path = buffer.path();
	let modified = buffer.modified();
	let readonly = buffer.is_readonly();
	let start_line = buffer.scroll_line;

	let title = path
		.as_ref()
		.map(|path| format!("xeno-iced - {}", path.display()))
		.unwrap_or_else(|| String::from("xeno-iced - [scratch]"));

	let mut rows = Vec::new();

	buffer.with_doc(|doc| {
		let content = doc.content();
		let total_lines = content.len_lines();
		let start = start_line.min(total_lines.saturating_sub(1));
		let end = start.saturating_add(MAX_VISIBLE_BUFFER_LINES).min(total_lines);

		rows.push(plain_line(format!(
			"path={} modified={} readonly={} lines={} showing={}..{}",
			path.as_ref().map_or_else(|| String::from("[scratch]"), |path| path.display().to_string()),
			modified,
			readonly,
			total_lines,
			start + 1,
			end,
		)));
		rows.push(plain_line(String::new()));

		for line_idx in start..end {
			let line = content.line(line_idx).to_string();
			let line = line.trim_end_matches(['\n', '\r']);
			rows.push(plain_line(format!("{:>6} {line}", line_idx + 1)));
		}

		if end < total_lines {
			let remaining = total_lines.saturating_sub(end);
			rows.push(plain_line(String::new()));
			rows.push(plain_line(format!("... {remaining} more lines not shown")));
		}
	});

	(title, rows)
}

fn snapshot_for_focused_view(editor: &mut Editor, focused: ViewId) -> Option<(String, Vec<RenderLine<'static>>)> {
	let title = editor
		.get_buffer(focused)?
		.path()
		.as_ref()
		.map(|path| format!("xeno-iced - {}", path.display()))
		.unwrap_or_else(|| String::from("xeno-iced - [scratch]"));

	let area = editor.view_area(focused);
	if area.width < 2 || area.height == 0 {
		return None;
	}

	let render_ctx = editor.render_ctx();
	let mut cache = std::mem::take(editor.render_cache_mut());
	let tab_width = editor.tab_width_for(focused);
	let cursorline = editor.cursorline_for(focused);

	let document_lines = editor.get_buffer(focused).map_or_else(
		|| vec![plain_line("no focused buffer")],
		|buffer| {
			let buffer_ctx = BufferRenderContext {
				theme: &render_ctx.theme,
				language_loader: &editor.config().language_loader,
				syntax_manager: editor.syntax_manager(),
				diagnostics: render_ctx.lsp.diagnostics_for(focused),
				diagnostic_ranges: render_ctx.lsp.diagnostic_ranges_for(focused),
			};

			let result = buffer_ctx.render_buffer(buffer, area, true, true, tab_width, cursorline, &mut cache);
			merge_render_lines(result.gutter, result.text)
		},
	);

	*editor.render_cache_mut() = cache;
	Some((title, document_lines))
}

fn merge_render_lines(gutter: Vec<RenderLine<'static>>, text: Vec<RenderLine<'static>>) -> Vec<RenderLine<'static>> {
	let row_count = gutter.len().max(text.len());
	let mut rows = Vec::with_capacity(row_count);

	for idx in 0..row_count {
		let mut spans = Vec::new();
		if let Some(gutter_line) = gutter.get(idx) {
			spans.extend(gutter_line.spans.iter().cloned());
		}
		if let Some(text_line) = text.get(idx) {
			spans.extend(text_line.spans.iter().cloned());
		}
		rows.push(RenderLine { spans, style: None });
	}

	rows
}

fn plain_line(content: impl Into<String>) -> RenderLine<'static> {
	RenderLine::from(vec![RenderSpan::styled(content.into(), Style::default())])
}

fn build_surface_summary_rows(
	overlay_kind: Option<OverlayControllerKind>,
	overlay_panes: &[OverlayPaneRenderTarget],
	completion_plan: Option<&CompletionRenderPlan>,
	snippet_plan: Option<&SnippetChoiceRenderPlan>,
	info_popup_plan: &[InfoPopupRenderTarget],
) -> Vec<String> {
	let mut rows = Vec::new();

	match overlay_kind {
		Some(kind) => {
			rows.push(format!("overlay={kind:?} panes={}", overlay_panes.len()));
			for pane in overlay_panes.iter().take(3) {
				rows.push(format!("  {:?} {}", pane.role, rect_brief(pane.rect)));
			}
			if overlay_panes.len() > 3 {
				rows.push(format!("  ... {} more panes", overlay_panes.len() - 3));
			}
		}
		None => rows.push(String::from("overlay=none")),
	}

	match completion_plan {
		Some(plan) => {
			let selected = plan
				.items
				.iter()
				.find(|item| item.selected)
				.map_or_else(|| String::from("-"), |item| item.label.clone());
			rows.push(format!(
				"completion=visible rows={} selected={} kind_col={} right_col={}",
				plan.items.len(),
				selected,
				plan.show_kind,
				plan.show_right
			));
		}
		None => rows.push(String::from("completion=hidden")),
	}

	match snippet_plan {
		Some(plan) => {
			let selected = plan
				.items
				.iter()
				.find(|item| item.selected)
				.map_or_else(|| String::from("-"), |item| item.option.clone());
			rows.push(format!("snippet_choice=visible rows={} selected={selected}", plan.items.len()));
		}
		None => rows.push(String::from("snippet_choice=hidden")),
	}

	if info_popup_plan.is_empty() {
		rows.push(String::from("info_popups=none"));
	} else {
		rows.push(format!("info_popups={}", info_popup_plan.len()));
		for popup in info_popup_plan.iter().take(2) {
			let anchor = match popup.anchor {
				InfoPopupRenderAnchor::Center => String::from("center"),
				InfoPopupRenderAnchor::Point { x, y } => format!("point@{x},{y}"),
			};
			rows.push(format!("  popup#{} {} {}x{}", popup.id.0, anchor, popup.content_width, popup.content_height));
		}
		if info_popup_plan.len() > 2 {
			rows.push(format!("  ... {} more popups", info_popup_plan.len() - 2));
		}
	}

	rows
}

fn build_completion_preview_rows(plan: Option<&CompletionRenderPlan>) -> Vec<String> {
	let Some(plan) = plan else {
		return vec![String::from("completion_rows=hidden")];
	};

	let mut rows = Vec::new();
	rows.push(format!(
		"completion_rows={} target_width={} kind_col={} right_col={}",
		plan.items.len(),
		plan.target_row_width,
		plan.show_kind,
		plan.show_right
	));

	for item in plan.items.iter().take(8) {
		let marker = if item.selected { ">" } else { " " };
		let mut row = format!("{marker} {}", item.label);
		if plan.show_kind {
			row.push_str(&format!("  [{:?}]", item.kind));
		}
		if plan.show_right
			&& let Some(right) = &item.right
		{
			row.push_str(&format!("  ({right})"));
		}
		rows.push(row);
	}

	if plan.items.len() > 8 {
		rows.push(format!("... {} more completion rows", plan.items.len() - 8));
	}

	rows
}

fn build_snippet_preview_rows(plan: Option<&SnippetChoiceRenderPlan>) -> Vec<String> {
	let Some(plan) = plan else {
		return vec![String::from("snippet_rows=hidden")];
	};

	let mut rows = Vec::new();
	rows.push(format!("snippet_rows={} target_width={}", plan.items.len(), plan.target_row_width));

	for item in plan.items.iter().take(8) {
		let marker = if item.selected { ">" } else { " " };
		rows.push(format!("{marker} {}", item.option));
	}

	if plan.items.len() > 8 {
		rows.push(format!("... {} more snippet rows", plan.items.len() - 8));
	}

	rows
}

fn compact_statusline(statusline: String) -> String {
	let mut compact = String::new();
	let mut last_was_space = false;

	for ch in statusline.chars() {
		if ch.is_whitespace() {
			if !last_was_space {
				compact.push(' ');
			}
			last_was_space = true;
		} else {
			compact.push(ch);
			last_was_space = false;
		}
	}

	compact.trim().to_string()
}

fn rect_brief(rect: Rect) -> String {
	format!("{}x{}@{},{}", rect.width, rect.height, rect.x, rect.y)
}

fn ime_preedit_label(preedit: Option<&str>) -> String {
	let Some(preedit) = preedit else {
		return String::from("-");
	};

	const MAX_CHARS: usize = 24;
	let total = preedit.chars().count();
	if total <= MAX_CHARS {
		return preedit.to_string();
	}

	let prefix: String = preedit.chars().take(MAX_CHARS).collect();
	format!("{prefix}...")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn ime_preedit_label_truncates_long_content() {
		assert_eq!(ime_preedit_label(None), "-");
		assert_eq!(ime_preedit_label(Some("short")), "short");
		assert_eq!(ime_preedit_label(Some("abcdefghijklmnopqrstuvwxyz")), "abcdefghijklmnopqrstuvwx...");
	}

	#[test]
	fn compact_statusline_collapses_whitespace_and_newlines() {
		assert_eq!(compact_statusline(String::from("  A   B\n\nC\tD  ")), "A B C D");
	}
}
