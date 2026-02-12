use xeno_editor::Editor;
use xeno_editor::ui::{PanelRenderTarget, UiManager};
use xeno_registry::themes::Theme;
use xeno_tui::layout::Position;

pub fn render_panels(ui: &mut UiManager, editor: &mut Editor, frame: &mut xeno_tui::Frame, plan: &[PanelRenderTarget], theme: &Theme) -> Option<Position> {
	let mut cursor: Option<Position> = None;
	for target in plan {
		if let Some(cursor_req) = ui.with_panel_mut(&target.id, |panel| panel.render(frame, target.area, editor, target.focused, theme))
			&& target.focused
			&& let Some(req) = cursor_req
		{
			cursor = Some(req.pos);
		}
	}
	cursor
}
