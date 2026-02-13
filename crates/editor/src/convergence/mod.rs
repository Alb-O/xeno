//! Cross-frontend render convergence testing.
//!
//! Defines digest types and collection strategies that replicate how TUI and Iced
//! call core render APIs. Tests verify both strategies produce identical digests
//! for the same editor state and viewport bounds.

#[cfg(test)]
mod tests;

use crate::Editor;
use crate::geometry::Rect;
use crate::info_popup::InfoPopupId;
use crate::overlay::WindowRole;

/// Summary of a rendered overlay pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaneDigest {
	pub role: WindowRole,
	pub rect: Rect,
	pub content_rect: Rect,
	pub gutter_width: u16,
	pub text_line_count: usize,
	pub gutter_line_count: usize,
}

/// Summary of a rendered info popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PopupDigest {
	pub id: InfoPopupId,
	pub rect: Rect,
	pub inner_rect: Rect,
	pub text_line_count: usize,
}

/// Summary of an overlay completion menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionDigest {
	pub rect: Rect,
	pub row_count: usize,
	pub selected_label: Option<String>,
	pub show_kind: bool,
	pub show_right: bool,
}

/// Summary of statusline rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusDigest {
	pub rows: u16,
	pub segment_count: usize,
}

/// Full render convergence digest for comparing frontend strategies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderConvergenceDigest {
	pub panes: Vec<PaneDigest>,
	pub popups: Vec<PopupDigest>,
	pub completion: Option<CompletionDigest>,
	pub status: StatusDigest,
}

/// Collects a digest using the TUI collection strategy.
///
/// Both frontends now consume identical core-owned view plan APIs,
/// so TUI and Iced digests use the same collection path.
pub(crate) fn collect_tui_digest(editor: &mut Editor, doc_bounds: Rect) -> RenderConvergenceDigest {
	collect_digest(editor, doc_bounds)
}

/// Collects a digest using the Iced collection strategy.
///
/// Both frontends now consume identical core-owned view plan APIs,
/// so this delegates to the same collection path as TUI.
pub(crate) fn collect_iced_digest(editor: &mut Editor, doc_bounds: Rect) -> RenderConvergenceDigest {
	collect_digest(editor, doc_bounds)
}

fn collect_digest(editor: &mut Editor, doc_bounds: Rect) -> RenderConvergenceDigest {
	let mut panes: Vec<PaneDigest> = editor
		.overlay_pane_view_plans()
		.into_iter()
		.map(|plan| PaneDigest {
			role: plan.role(),
			rect: plan.rect(),
			content_rect: plan.content_rect(),
			gutter_width: plan.gutter_rect().width,
			text_line_count: plan.text().len(),
			gutter_line_count: plan.gutter().len(),
		})
		.collect();
	panes.sort_by_key(|p| (p.rect.y, p.rect.x, p.rect.width, p.rect.height));

	let mut popups: Vec<PopupDigest> = editor
		.info_popup_view_plans(doc_bounds)
		.into_iter()
		.map(|plan| PopupDigest {
			id: plan.id(),
			rect: plan.rect(),
			inner_rect: plan.inner_rect(),
			text_line_count: plan.text().len(),
		})
		.collect();
	popups.sort_by_key(|p| p.id.0);

	let completion = editor.overlay_completion_menu_target().map(|target| {
		let selected = target.plan.items.iter().find(|item| item.selected).map(|item| item.label.clone());
		CompletionDigest {
			rect: target.rect,
			row_count: target.plan.items.len(),
			selected_label: selected,
			show_kind: target.plan.show_kind,
			show_right: target.plan.show_right,
		}
	});

	let status = StatusDigest {
		rows: editor.statusline_rows(),
		segment_count: editor.statusline_render_plan().len(),
	};

	RenderConvergenceDigest {
		panes,
		popups,
		completion,
		status,
	}
}
