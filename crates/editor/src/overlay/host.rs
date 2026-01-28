use xeno_primitives::Mode;
use xeno_tui::layout::Rect;

use super::CloseReason;
use super::session::OverlaySession;
use crate::buffer::ViewId;
use crate::impls::{Editor, FocusTarget};
use crate::window::Window;

pub struct OverlayHost;

impl OverlayHost {
	pub fn create_input_buffer(ed: &mut Editor) -> ViewId {
		ed.state.core.buffers.create_scratch()
	}

	pub fn setup_session(
		ed: &mut Editor,
		controller: &dyn super::OverlayController,
	) -> Option<OverlaySession> {
		let spec = controller.ui_spec(ed);
		let screen = match (ed.state.viewport.width, ed.state.viewport.height) {
			(Some(w), Some(h)) => Rect::new(0, 0, w, h),
			_ => return None,
		};

		let origin_focus = ed.state.focus.clone();
		let origin_view = ed.focused_view();
		let origin_mode = ed.focused_buffer().input.mode();

		let mut session = OverlaySession {
			windows: Vec::new(),
			buffers: Vec::new(),
			input: ViewId(0), // Placeholder
			origin_focus,
			origin_mode,
			origin_view,
			capture: Default::default(),
			status: Default::default(),
		};

		// Create primary input
		let input_buffer = Self::create_input_buffer(ed);
		session.input = input_buffer;
		session.buffers.push(input_buffer);

		let mut roles = std::collections::HashMap::new();
		let rect = spec.rect.resolve(screen, &roles);
		roles.insert(super::WindowRole::Input, rect);

		let window_id = ed.create_floating_window(input_buffer, rect, spec.style);
		session.windows.push(window_id);

		if let Some(Window::Floating(float)) = ed.state.windows.get_mut(window_id) {
			float.sticky = true;
			float.dismiss_on_blur = true;
			float.gutter = spec.gutter;
		}

		// Focus input
		ed.state.focus = FocusTarget::Buffer {
			window: window_id,
			buffer: input_buffer,
		};
		ed.state
			.core
			.buffers
			.get_buffer_mut(input_buffer)
			.unwrap()
			.input
			.set_mode(Mode::Insert);

		Some(session)
	}

	pub fn cleanup_session(
		ed: &mut Editor,
		controller: &mut dyn super::OverlayController,
		mut session: OverlaySession,
		reason: CloseReason,
	) {
		controller.on_close(ed, &mut session, reason);

		if reason != CloseReason::Commit {
			session.restore_all(ed);
		}

		for window_id in session.windows {
			ed.close_floating_window(window_id);
		}
		for buffer_id in session.buffers {
			ed.state.core.buffers.remove_buffer(buffer_id);
		}

		// Restore focus
		ed.state.focus = session.origin_focus;
		if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(session.origin_view) {
			buffer.input.set_mode(session.origin_mode);
		}

		ed.state.frame.needs_redraw = true;
	}
}
