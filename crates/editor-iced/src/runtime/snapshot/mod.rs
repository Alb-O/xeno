use xeno_editor::Editor;
use xeno_editor::render_api::{
	CompletionRenderPlan, DocumentViewPlan, InfoPopupRenderTarget, OverlayControllerKind, OverlayPaneRenderTarget, Rect, RenderLine, SeparatorJunctionTarget,
	SeparatorRenderTarget, SnippetChoiceRenderPlan, StatuslineRenderSegment, SurfaceStyle, WindowRole,
};

#[derive(Debug, Default)]
pub(crate) struct Snapshot {
	pub(crate) title: String,
	pub(crate) header: HeaderSnapshot,
	pub(crate) statusline_segments: Vec<StatuslineRenderSegment>,
	pub(crate) document_views: Vec<DocumentViewPlan>,
	pub(crate) separators: Vec<SeparatorRenderTarget>,
	pub(crate) junctions: Vec<SeparatorJunctionTarget>,
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
	pub(crate) overlay_pane_views: Vec<OverlayPaneViewSnapshot>,
	pub(crate) completion_plan: Option<CompletionRenderPlan>,
	pub(crate) snippet_plan: Option<SnippetChoiceRenderPlan>,
	pub(crate) info_popup_plan: Vec<InfoPopupRenderTarget>,
	pub(crate) info_popup_views: Vec<InfoPopupViewSnapshot>,
}

/// Pre-rendered overlay pane with resolved geometry and content lines.
#[derive(Debug, Clone)]
pub(crate) struct OverlayPaneViewSnapshot {
	pub(crate) role: WindowRole,
	pub(crate) rect: Rect,
	pub(crate) content_rect: Rect,
	pub(crate) style: SurfaceStyle,
	pub(crate) gutter_rect: Rect,
	pub(crate) text_rect: Rect,
	pub(crate) gutter: Vec<RenderLine<'static>>,
	pub(crate) text: Vec<RenderLine<'static>>,
}

/// Pre-rendered info popup with resolved geometry and content lines.
#[derive(Debug, Clone)]
pub(crate) struct InfoPopupViewSnapshot {
	/// Outer rect (background/clear area).
	pub(crate) rect: Rect,
	/// Inner rect after padding (where content is drawn).
	pub(crate) inner_rect: Rect,
	pub(crate) gutter_rect: Rect,
	pub(crate) text_rect: Rect,
	pub(crate) gutter: Vec<RenderLine<'static>>,
	pub(crate) text: Vec<RenderLine<'static>>,
}

pub(crate) fn build_snapshot(editor: &mut Editor, ime_preedit: Option<&str>, doc_bounds: Option<Rect>) -> Snapshot {
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
	let title = editor.focused_document_title();

	let document_views = doc_bounds.map(|bounds| editor.document_view_plans(bounds)).unwrap_or_default();
	let sep_scene = doc_bounds.map(|bounds| editor.separator_scene_plan(bounds));
	let separators = sep_scene.as_ref().map(|s| s.separators().to_vec()).unwrap_or_default();
	let junctions = sep_scene.as_ref().map(|s| s.junctions().to_vec()).unwrap_or_default();

	let overlay_pane_views = build_overlay_pane_views(editor);
	let info_popup_views = doc_bounds.map(|bounds| build_info_popup_views(editor, bounds)).unwrap_or_default();

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
		document_views,
		separators,
		junctions,
		surface: SurfaceSnapshot {
			overlay_kind,
			overlay_panes,
			overlay_pane_views,
			completion_plan,
			snippet_plan,
			info_popup_plan,
			info_popup_views,
		},
	}
}

fn build_overlay_pane_views(editor: &mut Editor) -> Vec<OverlayPaneViewSnapshot> {
	editor
		.overlay_pane_view_plans()
		.into_iter()
		.map(|plan| OverlayPaneViewSnapshot {
			role: plan.role(),
			rect: plan.rect(),
			content_rect: plan.content_rect(),
			style: plan.style().clone(),
			gutter_rect: plan.gutter_rect(),
			text_rect: plan.text_rect(),
			gutter: plan.gutter().to_vec(),
			text: plan.text().to_vec(),
		})
		.collect()
}

fn build_info_popup_views(editor: &mut Editor, bounds: Rect) -> Vec<InfoPopupViewSnapshot> {
	editor
		.info_popup_view_plans(bounds)
		.into_iter()
		.map(|plan| InfoPopupViewSnapshot {
			rect: plan.rect(),
			inner_rect: plan.inner_rect(),
			gutter_rect: plan.gutter_rect(),
			text_rect: plan.text_rect(),
			gutter: plan.gutter().to_vec(),
			text: plan.text().to_vec(),
		})
		.collect()
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
