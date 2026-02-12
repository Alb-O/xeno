use xeno_editor::Editor;
use xeno_tui::layout::Rect;

use crate::layer::SceneBuilder;
use crate::scene::{SurfaceKind, SurfaceOp};

pub fn visible(ed: &Editor) -> bool {
	ed.snippet_choice_popup_visible()
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(SurfaceKind::SnippetChoicePopup, 45, doc_area, SurfaceOp::SnippetChoicePopup, false);
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame) {
	ed.render_snippet_choice_popup(frame);
}
