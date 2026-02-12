use xeno_editor::Editor;
use xeno_editor::ui::UiManager;
use xeno_editor::ui::dock::DockLayout;
use xeno_registry::themes::Theme;
use xeno_tui::layout::Position;

pub fn render_panels(ui: &mut UiManager, editor: &mut Editor, frame: &mut xeno_tui::Frame, layout: &DockLayout, theme: &Theme) -> Option<Position> {
	let mut cursor: Option<Position> = None;
	for (id, area) in &layout.panel_areas {
		let focused = ui.is_panel_focused(id);
		if let Some(cursor_req) = ui.with_panel_mut(id, |panel| panel.render(frame, *area, editor, focused, theme))
			&& focused
			&& let Some(req) = cursor_req
		{
			cursor = Some(req.pos);
		}
	}
	cursor
}
