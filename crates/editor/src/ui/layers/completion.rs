use xeno_tui::layout::Rect;

use crate::impls::Editor;
use crate::ui::layer::SceneBuilder;
use crate::ui::scene::{SurfaceKind, SurfaceOp};

pub fn visible(ed: &Editor) -> bool {
	#[cfg(not(feature = "lsp"))]
	{
		let _ = ed;
		return false;
	}

	#[cfg(feature = "lsp")]
	{
		use crate::completion::CompletionState;

		let completions = ed
			.overlays()
			.get::<CompletionState>()
			.cloned()
			.unwrap_or_default();
		if !completions.active || completions.items.is_empty() {
			return false;
		}

		let Some(menu_state) = ed
			.overlays()
			.get::<crate::lsp::LspMenuState>()
			.and_then(|state| state.active())
		else {
			return false;
		};

		let buffer_id = match menu_state {
			crate::lsp::LspMenuKind::Completion { buffer_id, .. } => *buffer_id,
			crate::lsp::LspMenuKind::CodeAction { buffer_id, .. } => *buffer_id,
		};

		buffer_id == ed.focused_view()
	}
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(
		SurfaceKind::CompletionPopup,
		40,
		doc_area,
		SurfaceOp::CompletionPopup,
		false,
	);
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame) {
	ed.render_completion_popup(frame);
}
