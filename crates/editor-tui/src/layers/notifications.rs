use xeno_editor::Editor;

pub fn render(ed: &mut Editor, doc_area: xeno_tui::layout::Rect, buffer: &mut xeno_tui::buffer::Buffer) {
	let mut notifications_area = doc_area;
	notifications_area.height = notifications_area.height.saturating_sub(1);
	notifications_area.width = notifications_area.width.saturating_sub(1);
	ed.notifications_mut().toast_manager_mut().render(notifications_area, buffer);
}
