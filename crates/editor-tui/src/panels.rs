use xeno_editor::Editor;
use xeno_editor::render::RenderCtx;
use xeno_editor::ui::ids::UTILITY_PANEL_ID;
use xeno_editor::ui::{PanelRenderTarget, UiManager};
use xeno_tui::layout::Position;

pub fn render_panels(ui: &mut UiManager, editor: &mut Editor, frame: &mut xeno_tui::Frame, plan: &[PanelRenderTarget], ctx: &RenderCtx) -> Option<Position> {
	let theme = &ctx.theme;
	let mut cursor: Option<Position> = None;
	for target in plan {
		if let Some(cursor_req) = ui.with_panel_mut(&target.id, |panel| panel.render(frame, target.area, editor, target.focused, theme))
			&& target.focused
			&& let Some(req) = cursor_req
		{
			cursor = Some(req.pos);
		}
		if target.id == UTILITY_PANEL_ID && editor.overlay_interaction().is_open() {
			crate::layers::modal_overlays::render_utility_panel_overlay(editor, frame, target.area, ctx);
		}
	}
	cursor
}
