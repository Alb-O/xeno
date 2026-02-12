use xeno_editor::Editor;
use xeno_editor::ui::layer::SceneBuilder;
use xeno_editor::ui::scene::{SurfaceKind, SurfaceOp};
use xeno_tui::layout::Rect;

pub fn visible(ed: &Editor) -> bool {
	ed.snippet_choice_popup_visible()
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(SurfaceKind::SnippetChoicePopup, 45, doc_area, SurfaceOp::SnippetChoicePopup, false);
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame) {
	ed.render_snippet_choice_popup(frame);
}
