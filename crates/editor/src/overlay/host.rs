use xeno_primitives::Mode;
use xeno_tui::layout::Rect;

use super::CloseReason;
use super::session::OverlaySession;
use crate::buffer::ViewId;
use crate::impls::{Editor, FocusTarget};
use crate::window::Window;

/// Low-level manager for UI resources used by overlays.
///
/// `OverlayHost` handles the creation and destruction of scratch buffers and
/// floating windows required for a modal interaction session.
pub struct OverlayHost;

impl OverlayHost {
	/// Allocates a new scratch buffer for overlay input or display.
	pub fn create_input_buffer(ed: &mut Editor) -> ViewId {
		ed.state.core.buffers.create_scratch()
	}

	/// Configures a new interaction session based on a controller's UI specification.
	///
	/// This method:
	/// 1. Captures the current editor state (focus, mode, cursor) for restoration.
	/// 2. Resolves the primary input window's rectangle.
	/// 3. Spawns auxiliary windows defined in the specification.
	/// 4. Sets up buffer-local options for spawned windows.
	///
	/// Returns `None` if the terminal viewport dimensions are not available.
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
		let input_rect = spec.rect.resolve(screen, &roles);
		roles.insert(super::WindowRole::Input, input_rect);

		let window_id = ed.create_floating_window(input_buffer, input_rect, spec.style);
		session.windows.push(window_id);

		if let Some(Window::Floating(float)) = ed.state.windows.get_mut(window_id) {
			float.sticky = true;
			float.dismiss_on_blur = true;
			float.gutter = spec.gutter;
		}

		// Create auxiliary windows
		for win_spec in spec.windows {
			let buffer_id = ed.state.core.buffers.create_scratch();
			session.buffers.push(buffer_id);

			if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(buffer_id) {
				for (k, v) in win_spec.buffer_options {
					let _ = buffer.local_options.set_by_kdl(&k, v);
				}
			}

			let rect = win_spec.rect.resolve(screen, &roles);
			roles.insert(win_spec.role, rect);

			let win_id = ed.create_floating_window(buffer_id, rect, win_spec.style);
			session.windows.push(win_id);

			if let Some(Window::Floating(float)) = ed.state.windows.get_mut(win_id) {
				float.sticky = win_spec.sticky;
				float.dismiss_on_blur = win_spec.dismiss_on_blur;
				float.gutter = win_spec.gutter;
			}
		}

		// Focus primary input window
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

	/// Cleans up session resources and restores the editor to its original state.
	///
	/// This method:
	/// 1. Notifies the controller about closure.
	/// 2. Restores cursor and selection state unless committed.
	/// 3. Closes all session windows and removes scratch buffers.
	/// 4. Restores focus and mode to the captured values.
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

		// Restore original state
		ed.state.focus = session.origin_focus;
		if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(session.origin_view) {
			buffer.input.set_mode(session.origin_mode);
		}

		ed.state.frame.needs_redraw = true;
	}
}
