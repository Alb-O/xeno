use xeno_editor::Editor;
use xeno_editor::completion::CompletionRenderPlan;
use xeno_editor::info_popup::InfoPopupRenderTarget;
use xeno_editor::overlay::{OverlayControllerKind, OverlayPaneRenderTarget};
use xeno_editor::render_api::RenderLine;
use xeno_editor::snippet::SnippetChoiceRenderPlan;
use xeno_editor::ui::StatuslineRenderSegment;

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
	let mode = editor.mode_name();
	let cursor_line = editor.cursor_line() + 1;
	let cursor_col = editor.cursor_col() + 1;
	let buffers = editor.buffer_count();
	let statusline_segments = editor.statusline_render_plan();
	let overlay_kind = editor.overlay_kind();
	let overlay_panes = editor.overlay_pane_render_plan();
	let completion_plan = editor.completion_popup_render_plan();
	let snippet_plan = editor.snippet_choice_render_plan();
	let info_popup_plan = editor.info_popup_render_plan();
	let document_plan = editor.focused_document_render_plan();

	Snapshot {
		title: document_plan.title,
		header: HeaderSnapshot {
			mode: mode.to_string(),
			cursor_line,
			cursor_col,
			buffers,
			ime_preedit: ime_preedit_label(ime_preedit),
		},
		statusline_segments,
		document_lines: document_plan.lines,
		surface: SurfaceSnapshot {
			overlay_kind,
			overlay_panes,
			completion_plan,
			snippet_plan,
			info_popup_plan,
		},
	}
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
