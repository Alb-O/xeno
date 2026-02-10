use std::collections::HashMap;

use xeno_primitives::Mode;
use xeno_tui::layout::Rect;

use super::CloseReason;
use super::session::{OverlayPane, OverlaySession};
use crate::buffer::ViewId;
use crate::impls::{Editor, FocusReason, FocusTarget};

/// Low-level manager for UI resources used by overlays.
///
/// `OverlayHost` handles the creation and destruction of scratch buffers and
/// pane metadata required for a modal interaction session.
pub struct OverlayHost;

impl OverlayHost {
	pub fn reflow_session(
		ed: &mut Editor,
		controller: &dyn super::OverlayController,
		session: &mut OverlaySession,
	) -> bool {
		let spec = controller.ui_spec(ed);
		let (w, h) = match (ed.state.viewport.width, ed.state.viewport.height) {
			(Some(w), Some(h)) => (w, h),
			_ => return false,
		};
		let screen = Rect::new(0, 0, w, h);

		let mut roles = HashMap::new();
		let input_rect = match spec.rect.resolve_opt(screen, &roles) {
			Some(rect) => rect,
			None => return false,
		};
		roles.insert(super::WindowRole::Input, input_rect);

		let mut resolved: HashMap<
			super::WindowRole,
			(
				Rect,
				crate::window::SurfaceStyle,
				crate::window::GutterSelector,
				bool,
				bool,
			),
		> = HashMap::new();
		resolved.insert(
			super::WindowRole::Input,
			(input_rect, spec.style, spec.gutter, true, true),
		);

		for win_spec in spec.windows {
			debug_assert!(
				!resolved.contains_key(&win_spec.role),
				"OverlayUiSpec contains duplicate WindowRole during reflow: {:?}",
				win_spec.role
			);
			let Some(rect) = win_spec.rect.resolve_opt(screen, &roles) else {
				continue;
			};
			roles.insert(win_spec.role, rect);
			resolved.insert(
				win_spec.role,
				(
					rect,
					win_spec.style,
					win_spec.gutter,
					win_spec.dismiss_on_blur,
					win_spec.sticky,
				),
			);
		}

		for pane in &mut session.panes {
			match resolved.get(&pane.role) {
				Some((rect, style, gutter, dismiss_on_blur, sticky)) => {
					pane.rect = *rect;
					pane.content_rect = crate::overlay::geom::pane_inner_rect(*rect, style);
					pane.style = style.clone();
					pane.gutter = *gutter;
					pane.dismiss_on_blur = *dismiss_on_blur;
					pane.sticky = *sticky;
				}
				None if pane.role == super::WindowRole::Input => {
					return false;
				}
				None => {
					pane.rect = Rect::new(0, 0, 0, 0);
					pane.content_rect = Rect::new(0, 0, 0, 0);
				}
			}
		}

		true
	}

	/// Allocates a new scratch buffer for overlay input or display.
	pub fn create_input_buffer(ed: &mut Editor) -> ViewId {
		ed.state.core.buffers.create_scratch()
	}

	/// Configures a new interaction session based on a controller's UI specification.
	///
	/// # Resource Management
	///
	/// This method enforces a "resolve-before-allocate" policy:
	/// 1. Window rectangles are resolved against the current screen dimensions.
	/// 2. If resolution fails (e.g., screen too small, missing anchor), the operation aborts
	///    before any buffers are created.
	/// 3. Only after valid geometry is confirmed are scratch buffers and panes allocated.
	///
	/// This prevents orphaned scratch buffers in cases of layout failure.
	///
	/// # Returns
	///
	/// Returns `None` if:
	/// - The terminal viewport dimensions are not available.
	/// - The primary input window geometry fails to resolve.
	pub fn setup_session(
		ed: &mut Editor,
		controller: &dyn super::OverlayController,
	) -> Option<OverlaySession> {
		let spec = controller.ui_spec(ed);
		let (w, h) = (ed.state.viewport.width?, ed.state.viewport.height?);
		let screen = Rect::new(0, 0, w, h);

		let mut roles = std::collections::HashMap::new();

		// 1. Resolve Input Geometry
		let input_rect = spec.rect.resolve_opt(screen, &roles)?;
		roles.insert(super::WindowRole::Input, input_rect);

		let origin_focus = ed.state.focus.clone();
		let origin_view = ed.focused_view();
		let origin_mode = ed.focused_buffer().input.mode();

		let mut session = OverlaySession {
			panes: Vec::with_capacity(1 + spec.windows.len()),
			buffers: Vec::with_capacity(1 + spec.windows.len()),
			input: ViewId(0), // Placeholder, replaced below
			origin_focus,
			origin_mode,
			origin_view,
			capture: Default::default(),
			status: Default::default(),
		};

		// 2. Allocate Primary Input
		let input_buffer = Self::create_input_buffer(ed);
		session.input = input_buffer;
		session.buffers.push(input_buffer);
		session.panes.push(OverlayPane {
			role: super::WindowRole::Input,
			buffer: input_buffer,
			rect: input_rect,
			content_rect: crate::overlay::geom::pane_inner_rect(input_rect, &spec.style),
			style: spec.style,
			gutter: spec.gutter,
			dismiss_on_blur: true,
			sticky: true,
		});

		// 3. Resolve & Allocate Auxiliary Windows
		for win_spec in spec.windows {
			debug_assert!(
				!roles.contains_key(&win_spec.role),
				"OverlayUiSpec contains duplicate WindowRole during setup: {:?}",
				win_spec.role
			);
			// Resolve rect FIRST to avoid wasteful buffer allocation
			let rect = match win_spec.rect.resolve_opt(screen, &roles) {
				Some(r) => r,
				None => continue,
			};
			roles.insert(win_spec.role, rect);

			let buffer_id = ed.state.core.buffers.create_scratch();
			session.buffers.push(buffer_id);

			if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(buffer_id) {
				for (k, v) in win_spec.buffer_options {
					let _ = buffer
						.local_options
						.set_by_kdl(&xeno_registry::db::OPTIONS, &k, v);
				}
			}

			session.panes.push(OverlayPane {
				role: win_spec.role,
				buffer: buffer_id,
				rect,
				content_rect: crate::overlay::geom::pane_inner_rect(rect, &win_spec.style),
				style: win_spec.style,
				gutter: win_spec.gutter,
				dismiss_on_blur: win_spec.dismiss_on_blur,
				sticky: win_spec.sticky,
			});
		}

		ed.set_focus(
			FocusTarget::Overlay {
				buffer: input_buffer,
			},
			FocusReason::Programmatic,
		);
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
	/// 3. Removes all session scratch buffers.
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

		session.teardown(ed);

		// Restore original state
		if reason != CloseReason::Blur {
			ed.state.focus = session.origin_focus;
		}
		if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(session.origin_view) {
			buffer.input.set_mode(session.origin_mode);
		}

		ed.state.frame.needs_redraw = true;
	}
}
