use xeno_editor::completion::CompletionRenderPlan;
use xeno_editor::info_popup::InfoPopupRenderTarget;
use xeno_editor::overlay::{OverlayControllerKind, OverlayPaneRenderTarget};
use xeno_editor::render_api::{BufferRenderContext, RenderLine, RenderSpan};
use xeno_editor::snippet::SnippetChoiceRenderPlan;
use xeno_editor::ui::StatuslineRenderSegment;
use xeno_editor::{Buffer, Editor, ViewId};
use xeno_primitives::Style;

const MAX_VISIBLE_BUFFER_LINES: usize = 500;

#[derive(Debug, Default)]
pub(crate) struct Snapshot {
	pub(crate) title: String,
	pub(crate) header: HeaderSnapshot,
	pub(crate) statusline_segments: Vec<StatuslineRenderSegment>,
	pub(crate) document_lines: Vec<RenderLine<'static>>,
	pub(crate) surface: SurfaceSnapshot,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HeaderSnapshot {
	pub(crate) mode: String,
	pub(crate) cursor_line: usize,
	pub(crate) cursor_col: usize,
	pub(crate) buffers: usize,
	pub(crate) ime_preedit: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SurfaceSnapshot {
	pub(crate) overlay_kind: Option<OverlayControllerKind>,
	pub(crate) overlay_panes: Vec<OverlayPaneRenderTarget>,
	pub(crate) completion_plan: Option<CompletionRenderPlan>,
	pub(crate) snippet_plan: Option<SnippetChoiceRenderPlan>,
	pub(crate) info_popup_plan: Vec<InfoPopupRenderTarget>,
}

pub(crate) fn build_snapshot(editor: &mut Editor, ime_preedit: Option<&str>) -> Snapshot {
	editor.ensure_syntax_for_buffers();

	let mode = editor.mode_name();
	let cursor_line = editor.cursor_line() + 1;
	let cursor_col = editor.cursor_col() + 1;
	let buffers = editor.buffer_count();
	let focused = editor.focused_view();
	let statusline_segments = editor.statusline_render_plan();
	let overlay_kind = editor.overlay_kind();
	let overlay_panes = editor.overlay_pane_render_plan();
	let completion_plan = editor.completion_popup_render_plan();
	let snippet_plan = editor.snippet_choice_render_plan();
	let info_popup_plan = editor.info_popup_render_plan();

	let (title, document_lines) = snapshot_for_focused_view(editor, focused).unwrap_or_else(|| {
		editor.get_buffer(focused).map_or_else(
			|| (String::from("xeno-iced"), vec![plain_line("no focused buffer")]),
			|buffer| snapshot_for_buffer(buffer),
		)
	});

	Snapshot {
		title,
		header: HeaderSnapshot {
			mode: mode.to_string(),
			cursor_line,
			cursor_col,
			buffers,
			ime_preedit: ime_preedit_label(ime_preedit),
		},
		statusline_segments,
		document_lines,
		surface: SurfaceSnapshot {
			overlay_kind,
			overlay_panes,
			completion_plan,
			snippet_plan,
			info_popup_plan,
		},
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
mod tests;
