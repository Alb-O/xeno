use xeno_registry::actions::BindingMode;
use xeno_registry::get_keymap_registry;
use xeno_tui::layout::Rect;

use crate::impls::Editor;
use crate::render::RenderCtx;
use crate::ui::layer::SceneBuilder;
use crate::ui::scene::{SurfaceKind, SurfaceOp};

pub fn visible(ed: &Editor) -> bool {
	let pending_keys = ed.buffer().input.pending_keys();
	if pending_keys.is_empty() {
		return false;
	}

	let binding_mode = match ed.buffer().input.mode() {
		xeno_primitives::Mode::Normal => BindingMode::Normal,
		_ => return false,
	};

	let binding = get_keymap_registry();
	let continuations = binding.continuations_with_kind(binding_mode, pending_keys);
	!continuations.is_empty()
}

pub fn push(builder: &mut SceneBuilder, doc_area: Rect) {
	builder.push(
		SurfaceKind::WhichKeyHud,
		80,
		doc_area,
		SurfaceOp::WhichKeyHud,
		false,
	);
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame, doc_area: Rect, ctx: &RenderCtx) {
	ed.render_whichkey_hud(frame, doc_area, ctx);
}
