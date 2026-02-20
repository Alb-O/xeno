use super::*;

#[tokio::test(flavor = "current_thread")]
async fn buffer_view_render_plan_renders_for_focused_view_area() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	let view = editor.focused_view();
	let area = editor.view_area(view);

	let plan = editor.buffer_view_render_plan(view, area, true, true).expect("render plan for focused view");
	assert!(!plan.text.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn buffer_view_render_plan_returns_none_for_missing_view() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);

	let area = Rect::new(0, 0, 80, 24);
	assert!(editor.buffer_view_render_plan(ViewId(u64::MAX), area, true, false).is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn buffer_view_render_plan_gutter_width_fits_area() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(40, 10);
	let view = editor.focused_view();
	let area = editor.view_area(view);

	let plan = editor.buffer_view_render_plan(view, area, true, true).expect("render plan for focused view");
	assert!(plan.gutter_width <= area.width);
}

#[tokio::test(flavor = "current_thread")]
async fn buffer_view_render_plan_with_gutter_renders_with_requested_policy() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(40, 10);
	let view = editor.focused_view();
	let area = editor.view_area(view);

	let plan = editor
		.buffer_view_render_plan_with_gutter(view, area, true, true, crate::window::GutterSelector::Registry)
		.expect("render plan for focused view");
	assert!(plan.gutter_width <= area.width);
}

#[tokio::test(flavor = "current_thread")]
async fn document_view_plans_returns_plans_after_resize() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	let doc_area = editor.doc_area();

	let plans = editor.document_view_plans(doc_area);
	assert!(!plans.is_empty(), "should have at least one view plan");
	assert!(!plans[0].text.is_empty(), "view plan should have rendered text");
}

#[tokio::test(flavor = "current_thread")]
async fn separator_render_targets_empty_for_single_view() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	let doc_area = editor.doc_area();

	let targets = editor.separator_render_targets(doc_area);
	assert!(targets.is_empty(), "single view should have no separators");
}

#[tokio::test(flavor = "current_thread")]
async fn buffer_view_render_plan_sets_rects_consistently() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(80, 24);
	let view = editor.focused_view();
	let area = editor.view_area(view);

	let plan = editor.buffer_view_render_plan(view, area, true, true).expect("render plan");

	// Gutter rect starts at area origin.
	assert_eq!(plan.gutter_rect.x, area.x);
	assert_eq!(plan.gutter_rect.y, area.y);
	assert_eq!(plan.gutter_rect.width, plan.gutter_width);
	assert_eq!(plan.gutter_rect.height, area.height);

	// Text rect starts right after gutter.
	assert_eq!(plan.text_rect.x, area.x + plan.gutter_width);
	assert_eq!(plan.text_rect.y, area.y);
	assert_eq!(plan.text_rect.width, area.width.saturating_sub(plan.gutter_width));
	assert_eq!(plan.text_rect.height, area.height);
}

#[tokio::test(flavor = "current_thread")]
async fn buffer_view_render_plan_text_rect_never_overflows() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(3, 3);
	let view = editor.focused_view();
	let area = editor.view_area(view);

	if let Some(plan) = editor.buffer_view_render_plan(view, area, true, true) {
		assert!(plan.text_rect.x >= area.x);
		assert!(plan.text_rect.right() <= area.right());
		assert!(plan.gutter_rect.right() <= area.right());
	}
}
