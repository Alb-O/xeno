//! View focus management.
//!
//! Focusing buffers and navigating between views.

use xeno_primitives::Mode;
use xeno_registry::{HookContext, HookEventData, ViewId, emit_sync_with as emit_hook_sync_with};

use super::Editor;
use crate::buffer::SpatialDirection;
use crate::window::{Window, WindowId};

/// Panel identifier used by focus targets.
pub type PanelId = String;

/// Identifies what has keyboard focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusTarget {
	Buffer { window: WindowId, buffer: ViewId },
	Panel(PanelId),
}

/// Reason for focus change (for hooks and debugging).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusReason {
	/// User clicked on target.
	Click,
	/// User used keybinding (e.g., Ctrl+P for command palette).
	Keybinding,
	/// Programmatic focus (e.g., opening a new window).
	Programmatic,
	/// Mouse hover (if focus-follows-mouse enabled).
	Hover,
}

/// Converts a buffer view to a hook-compatible view ID.
fn hook_view_id(view: ViewId) -> ViewId {
	ViewId::text(view.0)
}

impl Editor {
	/// Focuses a specific view explicitly (user action like click or keybinding).
	///
	/// Returns true if the view exists and was focused.
	/// Explicit focus can override sticky focus and will close dockables.
	pub fn focus_view(&mut self, view: ViewId) -> bool {
		let window_id = self.state.windows.base_id();
		self.focus_buffer_in_window(window_id, view, true)
	}

	/// Focuses a specific view implicitly (mouse hover).
	///
	/// Returns true if the view exists and was focused.
	/// Respects sticky focus - won't steal focus from sticky views.
	pub fn focus_view_implicit(&mut self, view: ViewId) -> bool {
		let current = self.focused_view();
		if current == view || self.state.frame.sticky_views.contains(&current) {
			return false;
		}
		let window_id = self.state.windows.base_id();
		self.focus_buffer_in_window(window_id, view, false)
	}

	/// Internal focus implementation, handling sticky views.
	pub(crate) fn focus_buffer_in_window(
		&mut self,
		window_id: WindowId,
		view: ViewId,
		explicit: bool,
	) -> bool {
		if self.state.core.buffers.get_buffer(view).is_none() {
			return false;
		}

		let old_focus = self.state.focus.clone();
		let old_view = self.focused_view();
		let base_window_id = self.state.windows.base_id();

		self.state.focus = FocusTarget::Buffer {
			window: window_id,
			buffer: view,
		};
		if window_id == base_window_id {
			self.base_window_mut().focused_buffer = view;
		}
		let _ = self.state.core.buffers.set_focused_view(view);
		self.state.frame.needs_redraw = true;

		if explicit && view != old_view {
			self.state.frame.sticky_views.remove(&old_view);
		}

		if view != old_view {
			emit_hook_sync_with(
				&HookContext::new(HookEventData::ViewFocusChanged {
					view_id: hook_view_id(view),
					prev_view_id: Some(hook_view_id(old_view)),
				}),
				&mut self.state.hook_runtime,
			);
		}

		let new_focus = self.state.focus.clone();
		self.handle_window_focus_change(old_focus, &new_focus);

		true
	}

	/// Focuses a specific buffer by ID.
	///
	/// Returns true if the buffer exists and was focused.
	pub fn focus_buffer(&mut self, id: ViewId) -> bool {
		self.focus_view(id)
	}

	/// Focuses the next view in the layout.
	pub fn focus_next_view(&mut self) {
		let next = self
			.state
			.layout
			.next_view(&self.base_window().layout, self.focused_view());
		self.focus_view(next);
	}

	/// Focuses the previous view in the layout.
	pub fn focus_prev_view(&mut self) {
		let prev = self
			.state
			.layout
			.prev_view(&self.base_window().layout, self.focused_view());
		self.focus_view(prev);
	}

	/// Focuses the next text buffer in the layout.
	pub fn focus_next_buffer(&mut self) {
		let current_id = self.focused_view();
		let next_id = self
			.state
			.layout
			.next_buffer(&self.base_window().layout, current_id);
		self.focus_buffer(next_id);
	}

	/// Focuses the previous text buffer in the layout.
	pub fn focus_prev_buffer(&mut self) {
		let current_id = self.focused_view();
		let prev_id = self
			.state
			.layout
			.prev_buffer(&self.base_window().layout, current_id);
		self.focus_buffer(prev_id);
	}

	/// Focuses the view in the given direction, using cursor position as tiebreaker.
	pub fn focus_direction(&mut self, direction: SpatialDirection) {
		let area = self.doc_area();
		let current = self.focused_view();
		let hint = self.cursor_screen_pos(direction, area);

		if let Some(target) = self.state.layout.view_in_direction(
			&self.base_window().layout,
			area,
			current,
			direction,
			hint,
		) {
			self.focus_view(target);
		}
	}

	pub(crate) fn sync_focus_from_ui(&mut self) {
		let old_focus = self.state.focus.clone();
		if let Some(panel_id) = self.state.ui.focused_panel_id() {
			self.state.focus = FocusTarget::Panel(panel_id.to_string());
		} else if matches!(self.state.focus, FocusTarget::Panel(_)) {
			let buffer = self.base_window().focused_buffer;
			self.state.focus = FocusTarget::Buffer {
				window: self.state.windows.base_id(),
				buffer,
			};
		}

		if old_focus != self.state.focus {
			let new_focus = self.state.focus.clone();
			self.handle_window_focus_change(old_focus, &new_focus);
		}
	}

	/// Returns cursor screen position along the perpendicular axis for directional hints.
	fn cursor_screen_pos(&self, direction: SpatialDirection, area: xeno_tui::layout::Rect) -> u16 {
		let buffer = self.buffer();
		let view_rect = self
			.state
			.layout
			.compute_view_areas(&self.base_window().layout, area)
			.into_iter()
			.find(|(v, _)| *v == self.focused_view())
			.map(|(_, r)| r)
			.unwrap_or(area);

		match direction {
			SpatialDirection::Left | SpatialDirection::Right => {
				let visible_line = buffer.cursor_line().saturating_sub(buffer.scroll_line);
				view_rect.y + (visible_line as u16).min(view_rect.height.saturating_sub(1))
			}
			SpatialDirection::Up | SpatialDirection::Down => {
				let gutter = buffer.gutter_width();
				view_rect.x
					+ gutter + (buffer.cursor_col() as u16)
					.min(view_rect.width.saturating_sub(gutter + 1))
			}
		}
	}

	/// Returns the current editing mode (Normal, Insert, Visual, etc.).
	pub fn mode(&self) -> Mode {
		self.buffer().input.mode()
	}

	/// Returns the display name for the current mode.
	pub fn mode_name(&self) -> &'static str {
		self.buffer().input.mode_name()
	}

	fn handle_window_focus_change(&mut self, old_focus: FocusTarget, new_focus: &FocusTarget) {
		let old_window = match old_focus {
			FocusTarget::Buffer { window, .. } => Some(window),
			FocusTarget::Panel(_) => None,
		};
		let new_window = match new_focus {
			FocusTarget::Buffer { window, .. } => Some(*window),
			FocusTarget::Panel(_) => None,
		};

		if old_window != new_window {
			if let Some(window) = old_window {
				emit_hook_sync_with(
					&HookContext::new(HookEventData::WindowFocusChanged {
						window_id: window.into(),
						focused: false,
					}),
					&mut self.state.hook_runtime,
				);
			}
			if let Some(window) = new_window {
				emit_hook_sync_with(
					&HookContext::new(HookEventData::WindowFocusChanged {
						window_id: window.into(),
						focused: true,
					}),
					&mut self.state.hook_runtime,
				);
			}
		}

		if let Some(window) = old_window
			&& old_window != new_window
		{
			let should_close = matches!(
				self.state.windows.get(window),
				Some(Window::Floating(floating)) if floating.dismiss_on_blur
			);
			if should_close {
				if self.state.overlay_system.interaction.is_open()
					&& self
						.state
						.overlay_system
						.interaction
						.active
						.as_ref()
						.is_some_and(|a| a.session.windows.contains(&window))
				{
					self.interaction_cancel();
					return;
				}
				self.close_floating_window(window);
			}
		}
	}
}
