//! View focus management.
//!
//! Focusing buffers and navigating between views.

use xeno_primitives::Mode;
use xeno_registry::HookEventData;
use xeno_registry::hooks::{HookContext, ViewId, emit_sync_with as emit_hook_sync_with};

use super::Editor;
use crate::buffer::SpatialDirection;
use crate::window::{Window, WindowId};

/// Panel identifier used by focus targets.
pub type PanelId = String;

/// Identifies what has keyboard focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusTarget {
	Buffer { window: WindowId, buffer: ViewId },
	Overlay { buffer: ViewId },
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

/// Focus epoch counter, incremented whenever focus or structure changes.
///
/// Used by async tasks to detect if their target view is still valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FocusEpoch(pub u64);

impl FocusEpoch {
	pub fn initial() -> Self {
		Self(0)
	}

	pub fn increment(&mut self) {
		self.0 = self.0.wrapping_add(1);
	}
}

/// A lease on a view, allowing async operations to gracefully handle view closure.
///
/// Background tasks should acquire a lease before starting work, then check if
/// the lease is still valid before applying results.
#[derive(Debug, Clone)]
pub struct ViewLease {
	/// Preferred view to apply operation to.
	pub preferred_view: ViewId,
	/// Document the operation applies to.
	pub doc: crate::buffer::DocumentId,
	/// Epoch at which the lease was acquired.
	pub epoch: FocusEpoch,
}

/// Converts a buffer view to a hook-compatible view ID.
fn hook_view_id(view: ViewId) -> ViewId {
	ViewId::text(view.0)
}

impl Editor {
	/// Checks if a window contains a specific view.
	///
	/// For BaseWindow: checks if view is in the layout tree.
	/// For FloatingWindow: checks if view matches the window's buffer.
	fn window_contains_view(&self, window_id: WindowId, view: ViewId) -> bool {
		match self.state.windows.get(window_id) {
			Some(Window::Base(base)) => self.state.layout.contains_view(&base.layout, view),
			Some(Window::Floating(floating)) => floating.buffer == view,
			None => false,
		}
	}

	/// Returns the first live buffer in the base window layout.
	///
	/// Scans the layout and returns the first view that exists in ViewManager.
	/// This ensures we don't return dead views that haven't been removed from layout yet.
	fn first_live_buffer_in_layout(&self) -> Option<ViewId> {
		let base_layout = &self.base_window().layout;
		self.state
			.layout
			.views(base_layout)
			.into_iter()
			.find(|&view| self.state.core.buffers.get_buffer(view).is_some())
	}

	/// Unified focus transition API. Normalizes target, updates state, emits hooks.
	///
	/// This is the single source of truth for all focus changes. All other focus
	/// methods should funnel through this.
	///
	/// Sticky focus is bypassed when the target must be normalized.
	///
	/// Returns true if focus changed, false if already focused or target invalid.
	pub(crate) fn set_focus(&mut self, requested: FocusTarget, reason: FocusReason) -> bool {
		let normalized = self.normalize_target(requested.clone());
		let did_normalize = normalized.as_ref() != Some(&requested);

		let effective = match normalized {
			Some(target) => target,
			None => {
				let scratch_id = self.state.core.buffers.create_scratch();
				let base_id = self.state.windows.base_id();
				self.base_window_mut().focused_buffer = scratch_id;
				FocusTarget::Buffer {
					window: base_id,
					buffer: scratch_id,
				}
			}
		};

		if effective == self.state.focus {
			return false;
		}

		let old_focus = self.state.focus.clone();
		let old_view = self.focused_view();

		let is_explicit = matches!(reason, FocusReason::Click | FocusReason::Keybinding);
		if !is_explicit
			&& !did_normalize
			&& let FocusTarget::Buffer { buffer, .. } = &effective
			&& self.state.frame.sticky_views.contains(&old_view)
			&& *buffer != old_view
		{
			return false;
		}

		self.state.focus = effective.clone();
		self.notify_overlay_event(crate::overlay::LayerEvent::FocusChanged {
			from: old_focus.clone(),
			to: effective.clone(),
		});

		if let FocusTarget::Buffer { window, buffer } = &effective
			&& *window == self.state.windows.base_id()
		{
			self.base_window_mut().focused_buffer = *buffer;
		}

		self.state.focus_epoch.increment();

		self.state.frame.needs_redraw = true;

		if is_explicit
			&& let FocusTarget::Buffer { buffer, .. } = &effective
			&& *buffer != old_view
		{
			self.state.frame.sticky_views.remove(&old_view);
		}

		let new_view = self.focused_view();
		if new_view != old_view {
			emit_hook_sync_with(
				&HookContext::new(HookEventData::ViewFocusChanged {
					view_id: hook_view_id(new_view),
					prev_view_id: Some(hook_view_id(old_view)),
				}),
				&mut self.state.hook_runtime,
			);
		}

		let old_focus_for_blur = old_focus.clone();
		self.handle_window_focus_change(old_focus, &effective);

		let overlay_was_open = self.state.overlay_system.interaction.is_open();
		let leaving_overlay = matches!(old_focus_for_blur, FocusTarget::Overlay { .. })
			&& !matches!(effective, FocusTarget::Overlay { .. });
		if overlay_was_open && leaving_overlay && !matches!(reason, FocusReason::Hover) {
			let mut interaction = std::mem::take(&mut self.state.overlay_system.interaction);
			interaction.close(self, crate::overlay::CloseReason::Blur);
			self.state.overlay_system.interaction = interaction;
		}

		true
	}

	/// Normalizes a focus target, returning a valid target or fallback.
	///
	/// Ensures:
	/// - Window exists
	/// - Buffer exists and is a member of the window's layout
	/// - Panel exists (if Panel target)
	///
	/// Falls back to base window's focused buffer, then any buffer in layout,
	/// then any buffer in ViewManager.
	///
	/// Returns None only if no buffers exist at all (caller should create scratch).
	fn normalize_target(&self, target: FocusTarget) -> Option<FocusTarget> {
		match target {
			FocusTarget::Buffer { window, buffer } => {
				if self.state.windows.get(window).is_none() {
					return self.fallback_buffer_focus();
				}

				if self.state.core.buffers.get_buffer(buffer).is_none() {
					return self.fallback_buffer_focus();
				}

				if !self.window_contains_view(window, buffer) {
					return self.fallback_buffer_focus();
				}

				Some(FocusTarget::Buffer { window, buffer })
			}
			FocusTarget::Overlay { buffer } => {
				if self.state.core.buffers.get_buffer(buffer).is_some() {
					Some(FocusTarget::Overlay { buffer })
				} else {
					self.fallback_buffer_focus()
				}
			}
			FocusTarget::Panel(ref panel_id) => {
				if self.state.ui.has_panel(panel_id) {
					Some(target)
				} else {
					self.fallback_buffer_focus()
				}
			}
		}
	}

	/// Returns a valid fallback buffer focus target, or None if no buffers exist.
	///
	/// Priority:
	/// 1. Base window's focused_buffer (if it exists AND is in base layout)
	/// 2. First live buffer in base window layout
	/// 3. Any buffer in ViewManager (only if layout is empty)
	///
	/// If None is returned, the caller should create a scratch buffer.
	fn fallback_buffer_focus(&self) -> Option<FocusTarget> {
		let base_id = self.state.windows.base_id();
		let base_focused = self.base_window().focused_buffer;

		if self.state.core.buffers.get_buffer(base_focused).is_some()
			&& self.window_contains_view(base_id, base_focused)
		{
			return Some(FocusTarget::Buffer {
				window: base_id,
				buffer: base_focused,
			});
		}

		if let Some(buffer) = self.first_live_buffer_in_layout() {
			return Some(FocusTarget::Buffer {
				window: base_id,
				buffer,
			});
		}

		if let Some(buffer) = self.state.core.buffers.buffer_ids().next() {
			return Some(FocusTarget::Buffer {
				window: base_id,
				buffer,
			});
		}

		None
	}

	/// Focuses a specific view explicitly (user action like click or keybinding).
	///
	/// Returns true if the view exists and was focused.
	/// Explicit focus can override sticky focus and will close dockables.
	pub fn focus_view(&mut self, view: ViewId) -> bool {
		let window_id = self.state.windows.base_id();
		let target = FocusTarget::Buffer {
			window: window_id,
			buffer: view,
		};
		self.set_focus(target, FocusReason::Click)
	}

	/// Focuses a specific view implicitly (mouse hover).
	///
	/// Returns true if the view exists and was focused.
	/// Respects sticky focus - won't steal focus from sticky views.
	pub fn focus_view_implicit(&mut self, view: ViewId) -> bool {
		let window_id = self.state.windows.base_id();
		let target = FocusTarget::Buffer {
			window: window_id,
			buffer: view,
		};
		self.set_focus(target, FocusReason::Hover)
	}

	/// Internal focus implementation for backwards compatibility.
	///
	/// Prefer calling `set_focus()` directly with appropriate FocusReason.
	pub(crate) fn focus_buffer_in_window(
		&mut self,
		window_id: WindowId,
		view: ViewId,
		explicit: bool,
	) -> bool {
		let target = FocusTarget::Buffer {
			window: window_id,
			buffer: view,
		};
		let reason = if explicit {
			FocusReason::Click
		} else {
			FocusReason::Programmatic
		};
		self.set_focus(target, reason)
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
		if let Some(panel_id) = self.state.ui.focused_panel_id() {
			let target = FocusTarget::Panel(panel_id.to_string());
			self.set_focus(target, FocusReason::Programmatic);
		} else if matches!(self.state.focus, FocusTarget::Panel(_)) {
			let buffer = self.base_window().focused_buffer;
			let target = FocusTarget::Buffer {
				window: self.state.windows.base_id(),
				buffer,
			};
			self.set_focus(target, FocusReason::Programmatic);
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

	/// Repairs focus invariants after structural changes.
	///
	/// Ensures focus points to a valid target, creates a scratch buffer if needed,
	/// and synchronizes derived caches.
	///
	/// Call this after structural changes (split closed, window removed, etc.).
	pub(crate) fn repair_invariants(&mut self) {
		let current = self.state.focus.clone();

		let effective = match self.normalize_target(current.clone()) {
			Some(t) => t,
			None => {
				self.set_focus(current, FocusReason::Programmatic);
				return;
			}
		};

		if effective != current {
			self.set_focus(effective, FocusReason::Programmatic);
			return;
		}

		if let FocusTarget::Buffer { window, buffer } = effective
			&& window == self.state.windows.base_id()
		{
			let win = self.base_window_mut();
			if win.focused_buffer != buffer {
				win.focused_buffer = buffer;
				self.state.focus_epoch.increment();
			}
		}
	}

	/// Creates a lease for the currently focused buffer.
	///
	/// Use this before starting an async operation. The lease can be used to
	/// safely resolve the view later, handling cases where the view was closed
	/// or repurposed.
	pub fn lease_focused_view(&self) -> Option<ViewLease> {
		if let FocusTarget::Buffer { buffer, .. } | FocusTarget::Overlay { buffer } =
			self.state.focus
			&& let Some(buf) = self.state.core.buffers.get_buffer(buffer)
		{
			return Some(ViewLease {
				preferred_view: buffer,
				doc: buf.document_id(),
				epoch: self.state.focus_epoch,
			});
		}
		None
	}

	/// Resolves a view lease to a valid view ID.
	///
	/// Returns `Some(view_id)` if the original view still exists and points to the
	/// same document, or if another view for the same document exists.
	pub fn resolve_lease(&self, lease: &ViewLease) -> Option<ViewId> {
		if let Some(buf) = self.state.core.buffers.get_buffer(lease.preferred_view)
			&& buf.document_id() == lease.doc
		{
			return Some(lease.preferred_view);
		}

		self.state.core.buffers.any_buffer_for_doc(lease.doc)
	}

	/// Asserts internal invariants are maintained.
	///
	/// Use this in tests and debug builds to catch corruption early.
	#[cfg(any(test, debug_assertions))]
	pub fn debug_assert_invariants(&self) {
		match &self.state.focus {
			FocusTarget::Buffer { window, buffer } => {
				assert!(
					self.state.windows.get(*window).is_some(),
					"focused window must exist"
				);
				assert!(
					self.state.core.buffers.get_buffer(*buffer).is_some(),
					"focused buffer must exist"
				);
				if *window == self.state.windows.base_id() {
					assert!(
						self.window_contains_view(*window, *buffer),
						"focused buffer must be in window layout"
					);
				}
			}
			FocusTarget::Overlay { buffer } => {
				assert!(
					self.state.core.buffers.get_buffer(*buffer).is_some(),
					"focused overlay buffer must exist"
				);
			}
			FocusTarget::Panel(_) => {}
		}

		let base_focused = self.base_window().focused_buffer;
		assert!(
			self.state.core.buffers.get_buffer(base_focused).is_some(),
			"base window return buffer must exist"
		);

		assert!(
			self.state.core.buffers.buffer_count() > 0,
			"must have at least one buffer"
		);
	}

	pub(crate) fn handle_window_focus_change(
		&mut self,
		old_focus: FocusTarget,
		new_focus: &FocusTarget,
	) {
		let old_window = match old_focus {
			FocusTarget::Buffer { window, .. } => Some(window),
			FocusTarget::Overlay { .. } => None,
			FocusTarget::Panel(_) => None,
		};
		let new_window = match new_focus {
			FocusTarget::Buffer { window, .. } => Some(*window),
			FocusTarget::Overlay { .. } => None,
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
				self.close_floating_window(window);
			}
		}
	}
}

#[cfg(test)]
mod tests;
