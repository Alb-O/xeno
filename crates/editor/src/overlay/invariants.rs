use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use xeno_primitives::Selection;
use xeno_tui::layout::Rect;

use crate::overlay::spec::RectPolicy;
use crate::overlay::{CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayStatus, OverlayUiSpec, WindowRole, WindowSpec};
use crate::window::GutterSelector;

struct ReflowTestOverlay;

impl OverlayController for ReflowTestOverlay {
	fn name(&self) -> &'static str {
		"ReflowTest"
	}

	fn ui_spec(&self, _ctx: &dyn OverlayContext) -> OverlayUiSpec {
		OverlayUiSpec {
			title: Some("ReflowTest".to_string()),
			gutter: GutterSelector::Prompt('>'),
			rect: RectPolicy::TopCenter {
				width_percent: 100,
				max_width: u16::MAX,
				min_width: 1,
				y_frac: (0, 1),
				height: 1,
			},
			style: crate::overlay::docked_prompt_style(),
			windows: vec![WindowSpec {
				role: WindowRole::List,
				rect: RectPolicy::Below(WindowRole::Input, 1, 1),
				style: crate::overlay::docked_prompt_style(),
				buffer_options: HashMap::new(),
				dismiss_on_blur: true,
				sticky: false,
				gutter: GutterSelector::Hidden,
			}],
		}
	}

	fn on_open(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession) {}

	fn on_input_changed(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _text: &str) {}

	fn on_commit<'a>(&'a mut self, _ctx: &'a mut dyn OverlayContext, _session: &'a mut OverlaySession) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		Box::pin(async {})
	}

	fn on_close(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _reason: CloseReason) {}
}

/// Must gate state restoration on captured buffer version matching.
///
/// - Enforced in: `OverlaySession::restore_all`
/// - Failure symptom: Stale cursor/selection state is restored over user's edits.
#[cfg_attr(test, test)]
pub(crate) fn test_versioned_restore() {
	let mut editor = crate::impls::Editor::new_scratch();
	let view = editor.focused_view();

	{
		let buffer = editor.state.core.buffers.get_buffer_mut(view).expect("focused buffer must exist");
		buffer.set_cursor_and_selection(0, Selection::single(0, 0));
	}

	let mut session = OverlaySession {
		panes: Vec::new(),
		buffers: Vec::new(),
		input: view,
		origin_focus: editor.focus().clone(),
		origin_mode: editor.mode(),
		origin_view: view,
		capture: Default::default(),
		status: OverlayStatus::default(),
	};
	session.capture_view(&editor, view);

	{
		let buffer = editor.state.core.buffers.get_buffer_mut(view).expect("focused buffer must exist");
		buffer.reset_content("changed");
		buffer.set_cursor_and_selection(3, Selection::single(3, 3));
	}

	session.restore_all(&mut editor);

	let buffer = editor.state.core.buffers.get_buffer(view).expect("focused buffer must exist");
	assert_eq!(buffer.cursor, 3);
	assert_eq!(buffer.selection, Selection::single(3, 3));
}

/// Must allow only one active modal session at a time.
///
/// - Enforced in: `OverlayManager::open`
/// - Failure symptom: Two modal overlays fight for focus and input.
#[cfg_attr(test, test)]
pub(crate) fn test_exclusive_modal() {
	let mut editor = crate::impls::Editor::new_scratch();
	editor.handle_window_resize(100, 40);

	assert!(editor.open_command_palette());
	assert!(editor.state.overlay_system.interaction.is_open());
	assert!(!editor.open_command_palette());
}

/// Must clamp resolved overlay areas to screen bounds.
///
/// - Enforced in: `RectPolicy::resolve_opt`
/// - Failure symptom: Overlay panes extend beyond screen bounds.
#[cfg_attr(test, test)]
pub(crate) fn test_rect_policy_clamps_to_screen() {
	let screen = Rect::new(0, 0, 100, 50);
	let roles = HashMap::new();

	let policy = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 80,
		min_width: 20,
		y_frac: (1, 4),
		height: 10,
	};
	let rect = policy.resolve_opt(screen, &roles).unwrap();
	assert!(rect.x + rect.width <= screen.x + screen.width);
	assert!(rect.y + rect.height <= screen.y + screen.height);

	let policy_low = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 80,
		min_width: 20,
		y_frac: (9, 10),
		height: 20,
	};
	let rect_low = policy_low.resolve_opt(screen, &roles).unwrap();
	assert!(rect_low.y + rect_low.height <= screen.y + screen.height, "rect must be shifted up to fit");

	let zero = Rect::new(0, 0, 0, 0);
	assert!(policy.resolve_opt(zero, &roles).is_none());
}

/// Must finalize all session buffers during teardown.
///
/// - Enforced in: `OverlaySession::teardown`
/// - Failure symptom: Scratch buffers leak after overlays close.
#[cfg_attr(test, test)]
pub(crate) fn test_session_teardown_finalizes_buffers() {
	let mut editor = crate::impls::Editor::new_scratch();
	let b1 = editor.state.core.buffers.create_scratch();
	let b2 = editor.state.core.buffers.create_scratch();
	let view = editor.focused_view();

	let mut session = OverlaySession {
		panes: Vec::new(),
		buffers: vec![b1, b2],
		input: b1,
		origin_focus: editor.focus().clone(),
		origin_mode: editor.mode(),
		origin_view: view,
		capture: Default::default(),
		status: OverlayStatus::default(),
	};

	session.teardown(&mut editor);

	assert!(session.buffers.is_empty());
	assert!(editor.state.core.buffers.get_buffer(b1).is_none());
	assert!(editor.state.core.buffers.get_buffer(b2).is_none());
}

/// Must reflow modal overlay panes on viewport resize.
///
/// - Enforced in: `OverlayManager::on_viewport_changed`
/// - Failure symptom: Open modals render with stale geometry after terminal resize.
#[cfg_attr(test, test)]
pub(crate) fn test_modal_reflow_on_resize() {
	let mut editor = crate::impls::Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let before = editor
		.state
		.overlay_system
		.interaction
		.active
		.as_ref()
		.and_then(|active| active.session.panes.first())
		.map(|pane| pane.rect)
		.expect("overlay pane should exist");

	editor.handle_window_resize(60, 20);

	let after = editor
		.state
		.overlay_system
		.interaction
		.active
		.as_ref()
		.and_then(|active| active.session.panes.first())
		.map(|pane| pane.rect)
		.expect("overlay pane should still exist");

	assert_ne!(before, after, "pane rect should reflow after resize");
	assert!(after.x + after.width <= 60);
	assert!(after.y + after.height <= 20);
}

/// Must clear auxiliary panes that cannot resolve after viewport shrink.
///
/// - Enforced in: `OverlayHost::reflow_session`
/// - Failure symptom: Auxiliary overlays keep stale geometry after resize.
#[cfg_attr(test, test)]
pub(crate) fn test_modal_reflow_clears_unresolved_aux_panes() {
	let mut editor = crate::impls::Editor::new_scratch();
	editor.handle_window_resize(100, 40);

	let mut interaction = std::mem::take(&mut editor.state.overlay_system.interaction);
	assert!(interaction.open(&mut editor, Box::new(ReflowTestOverlay)));
	editor.state.overlay_system.interaction = interaction;

	let active = editor.state.overlay_system.interaction.active.as_ref().expect("overlay should be open");
	let input_rect = active
		.session
		.panes
		.iter()
		.find(|pane| pane.role == WindowRole::Input)
		.expect("input pane must exist")
		.rect;
	let list_rect = active
		.session
		.panes
		.iter()
		.find(|pane| pane.role == WindowRole::List)
		.expect("list pane must exist")
		.rect;
	assert!(input_rect.width > 0 && input_rect.height > 0);
	assert!(list_rect.width > 0 && list_rect.height > 0);

	editor.handle_window_resize(100, 2);

	let active = editor.state.overlay_system.interaction.active.as_ref().expect("overlay should remain open");
	let input_rect = active
		.session
		.panes
		.iter()
		.find(|pane| pane.role == WindowRole::Input)
		.expect("input pane must exist")
		.rect;
	let list_rect = active
		.session
		.panes
		.iter()
		.find(|pane| pane.role == WindowRole::List)
		.expect("list pane must exist")
		.rect;

	assert!(input_rect.width > 0 && input_rect.height > 0);
	assert_eq!(list_rect, Rect::new(0, 0, 0, 0));
}

/// Must restore origin focus on forced overlay close.
///
/// - Enforced in: `OverlayHost::cleanup_session`
/// - Failure symptom: Focus remains on stale overlay target after forced close.
#[cfg_attr(test, test)]
pub(crate) fn test_forced_close_restores_origin_focus() {
	let mut editor = crate::impls::Editor::new_scratch();
	editor.handle_window_resize(100, 40);
	assert!(editor.open_command_palette());

	let origin_focus = editor
		.state
		.overlay_system
		.interaction
		.active
		.as_ref()
		.map(|active| active.session.origin_focus.clone())
		.expect("overlay should be open");

	let mut interaction = std::mem::take(&mut editor.state.overlay_system.interaction);
	interaction.close(&mut editor, CloseReason::Forced);
	editor.state.overlay_system.interaction = interaction;

	assert_eq!(*editor.focus(), origin_focus);
}

/// Must keep window manager state fixed to the base window for modal UI paths.
///
/// - Enforced in: `OverlayHost::setup_session`
/// - Failure symptom: Overlay UI mutates window manager state unexpectedly.
#[cfg_attr(test, test)]
pub(crate) fn test_modal_ui_keeps_single_base_window() {
	let mut editor = crate::impls::Editor::new_scratch();
	editor.handle_window_resize(100, 40);

	assert_eq!(editor.state.windows.windows().count(), 1);
	assert!(editor.open_command_palette());
	assert_eq!(editor.state.windows.windows().count(), 1);

	editor.interaction_cancel();
	assert_eq!(editor.state.windows.windows().count(), 1);
}
