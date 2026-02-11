use xeno_tui::layout::Rect;

use crate::impls::Editor;
use crate::snippet::SnippetChoiceOverlay;
use crate::ui::layer::SceneBuilder;
use crate::ui::scene::{SurfaceKind, SurfaceOp};

pub fn visible(ed: &Editor) -> bool {
	if ed.state.overlay_system.interaction.is_open() {
		return false;
	}

	ed.overlays()
		.get::<SnippetChoiceOverlay>()
		.is_some_and(|overlay| overlay.active && overlay.buffer_id == ed.focused_view() && !overlay.options.is_empty())
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(SurfaceKind::SnippetChoicePopup, 45, doc_area, SurfaceOp::SnippetChoicePopup, false);
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame) {
	ed.render_snippet_choice_popup(frame);
}
