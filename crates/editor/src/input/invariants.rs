use xeno_primitives::{Key, KeyCode, Modifiers, MouseButton, MouseEvent};

use crate::Editor;
use crate::impls::FocusTarget;
use crate::input::protocol::{InputDispatchCmd, InputDispatchEvt, InputLocalEffect};

fn mouse_press(col: u16, row: u16) -> MouseEvent {
	MouseEvent::Press {
		button: MouseButton::Left,
		row,
		col,
		modifiers: Modifiers::NONE,
	}
}

fn mouse_drag(col: u16, row: u16) -> MouseEvent {
	MouseEvent::Drag {
		button: MouseButton::Left,
		row,
		col,
		modifiers: Modifiers::NONE,
	}
}

fn mouse_release(col: u16, row: u16) -> MouseEvent {
	MouseEvent::Release { row, col }
}

fn first_separator_cell(editor: &Editor) -> (u16, u16) {
	let doc_area = editor.doc_area();
	let separator_positions = editor.state.layout.separator_positions(&editor.base_window().layout, doc_area);
	let (_, _, rect) = separator_positions.into_iter().next().expect("layout should expose at least one separator");
	(rect.x, rect.y)
}

/// Must produce typed local-effect events for key command envelopes.
///
/// * Enforced in: `Editor::dispatch_input_cmd`
/// * Failure symptom: runtime cannot route key commands through typed input protocol.
#[tokio::test]
async fn test_input_dispatch_cmd_key_produces_local_effect_event() {
	let mut editor = Editor::new_scratch();
	let events = editor.dispatch_input_cmd(InputDispatchCmd::Key(Key::char('x'))).await;
	assert!(events.iter().any(|event| matches!(
		event,
		InputDispatchEvt::LocalEffectRequested(InputLocalEffect::DispatchKey(key)) if *key == Key::char('x')
	)));
	assert!(events.iter().any(|event| matches!(event, InputDispatchEvt::Consumed)));
}

/// Must let runtime apply deferred overlay commit through typed input events.
///
/// * Enforced in: `Editor::apply_input_dispatch_evt`
/// * Failure symptom: deferred overlay commits emitted by input protocol are dropped.
#[tokio::test]
async fn test_input_overlay_commit_event_enqueues_runtime_work() {
	let mut editor = Editor::new_scratch();
	editor.apply_input_dispatch_evt(InputDispatchEvt::OverlayCommitDeferred).await;
	assert!(editor.has_runtime_overlay_commit_work());
}

/// Must allow active overlay interaction to consume Enter and defer commit.
///
/// * Enforced in: `Editor::handle_key_active`
/// * Failure symptom: modal commit executes re-entrantly in key handling.
#[tokio::test]
async fn test_overlay_enter_queues_deferred_commit() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.handle_key(Key::new(KeyCode::Enter)).await;
	assert!(editor.has_runtime_overlay_commit_work());
	assert!(editor.overlay_kind().is_some());
}

/// Must keep overlay focus when keys are handled by active modal interaction.
///
/// * Enforced in: `Editor::handle_key_active`, overlay controller key handlers
/// * Failure symptom: typed modal input leaks into base editor state.
#[tokio::test]
async fn test_modal_key_keeps_overlay_focus() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.handle_key(Key::char('s')).await;

	assert!(editor.overlay_kind().is_some());
	assert!(matches!(editor.focus(), FocusTarget::Overlay { .. }));
}

/// Must dismiss modal overlays on outside click.
///
/// * Enforced in: `Editor::handle_mouse_in_doc_area`
/// * Failure symptom: overlays trap focus and cannot be dismissed by pointer.
#[tokio::test]
async fn test_click_outside_modal_closes_overlay() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let _ = editor.handle_mouse(mouse_press(0, 0)).await;

	assert!(editor.overlay_kind().is_none());
}

/// Must prioritize active separator drags over lower-priority selection release routes.
///
/// * Enforced in: `mouse_handling::routing::decide_mouse_route`
/// * Failure symptom: releasing during separator drag unexpectedly clears text-selection origin state.
#[tokio::test]
async fn test_active_drag_precedes_selection_release_route() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	editor.split_vertical_with_clone().expect("split should succeed");

	let (sep_x, sep_y) = first_separator_cell(&editor);
	let _ = editor.handle_mouse(mouse_press(sep_x, sep_y)).await;
	assert!(editor.state.layout.drag_state().is_some());

	let origin_view = editor.focused_view();
	let origin_area = editor.view_area(origin_view);
	editor.state.layout.text_selection_origin = Some((origin_view, origin_area));

	let _ = editor.handle_mouse(mouse_release(sep_x, sep_y)).await;

	assert!(editor.state.layout.drag_state().is_none());
	assert_eq!(editor.state.layout.text_selection_origin.map(|(view, _)| view), Some(origin_view));
}

/// Must route overlay hits before separator/view routing.
///
/// * Enforced in: `mouse_handling::routing::decide_mouse_route`
/// * Failure symptom: clicks inside modal panes blur overlay focus and interact with underlying views.
#[tokio::test]
async fn test_overlay_hit_precedes_separator_and_view_routing() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	editor.split_vertical_with_clone().expect("split should succeed");
	assert!(editor.open_command_palette());

	let pane = editor
		.state
		.overlay_system
		.interaction()
		.active()
		.and_then(|active| active.session.panes.first())
		.expect("overlay pane should exist");

	let _ = editor.handle_mouse(mouse_press(pane.rect.x, pane.rect.y)).await;

	assert!(editor.state.overlay_system.interaction().is_open());
	assert!(matches!(editor.focus(), FocusTarget::Overlay { .. }));
}

/// Must confine text-selection drag updates to the origin view.
///
/// * Enforced in: `Editor::handle_mouse_in_doc_area`, `mouse_handling::effects::apply_text_selection_drag_route`
/// * Failure symptom: drag across splits steals focus and updates selection in the wrong view.
#[tokio::test]
async fn test_text_selection_drag_stays_in_origin_view() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	editor.split_vertical_with_clone().expect("split should succeed");

	let doc_area = editor.doc_area();
	let mut view_areas = editor.state.layout.compute_view_areas(&editor.base_window().layout, doc_area);
	view_areas.sort_by_key(|(_, area)| area.x);
	let (left_view, left_area) = view_areas.first().copied().expect("left view should exist");
	let (_right_view, right_area) = view_areas.get(1).copied().expect("right view should exist");

	assert!(editor.focus_view(left_view));
	let press_x = left_area.x.saturating_add(1);
	let press_y = left_area.y.saturating_add(1);
	let _ = editor.handle_mouse(mouse_press(press_x, press_y)).await;
	assert_eq!(editor.state.layout.text_selection_origin.map(|(view, _)| view), Some(left_view));

	let drag_x = right_area.x.saturating_add(1);
	let drag_y = right_area.y.saturating_add(1);
	let _ = editor.handle_mouse(mouse_drag(drag_x, drag_y)).await;

	assert_eq!(editor.state.layout.text_selection_origin.map(|(view, _)| view), Some(left_view));
	assert!(matches!(
		editor.focus(),
		FocusTarget::Buffer {
			buffer,
			..
		} if *buffer == left_view
	));
}

/// Must cancel stale separator drags before attempting resize.
///
/// * Enforced in: `mouse_handling::effects::apply_separator_drag_route`
/// * Failure symptom: stale drags keep running after layout revision changes.
#[tokio::test]
async fn test_stale_drag_cancels_before_resize() {
	let mut editor = Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	editor.split_vertical_with_clone().expect("split should succeed");

	let (sep_x, sep_y) = first_separator_cell(&editor);
	let _ = editor.handle_mouse(mouse_press(sep_x, sep_y)).await;
	assert!(editor.state.layout.drag_state().is_some());

	editor
		.split_horizontal_with_clone()
		.expect("split should change layout revision while drag is active");

	let _ = editor.handle_mouse(mouse_drag(sep_x, sep_y)).await;

	assert!(editor.state.layout.drag_state().is_none());
}
