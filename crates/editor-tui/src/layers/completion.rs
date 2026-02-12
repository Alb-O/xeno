use xeno_editor::Editor;
use xeno_editor::ui::layer::SceneBuilder;
use xeno_editor::ui::scene::{SurfaceKind, SurfaceOp};
use xeno_tui::layout::Rect;

pub fn visible(ed: &Editor) -> bool {
	ed.completion_popup_visible()
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(SurfaceKind::CompletionPopup, 40, doc_area, SurfaceOp::CompletionPopup, false);
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame) {
	ed.render_completion_popup(frame);
}
