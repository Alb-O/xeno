//! Mouse event handling.
//!
//! Processing mouse input for text selection and separator dragging.

use termina::event::MouseEventKind;
use xeno_input::KeyResult;
use xeno_primitives::Selection;

use crate::editor::{Editor, FocusTarget};
use crate::window::Window;

impl Editor {
	/// Processes a mouse event, returning true if the event triggered a quit.
	pub async fn handle_mouse(&mut self, mouse: termina::event::MouseEvent) -> bool {
		let width = self.viewport.width.unwrap_or(80);
		let height = self.viewport.height.unwrap_or(24);

		// Main area excludes status line (1 row)
		let main_height = height.saturating_sub(1);
		let main_area = xeno_tui::layout::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};

		let mut ui = std::mem::take(&mut self.ui);
		let dock_layout = ui.compute_layout(main_area);

		if ui.handle_mouse(self, mouse, &dock_layout) {
			if ui.take_wants_redraw() {
				self.frame.needs_redraw = true;
			}
			self.ui = ui;
			self.sync_focus_from_ui();
			return false;
		}
		if ui.take_wants_redraw() {
			self.frame.needs_redraw = true;
		}
		self.ui = ui;
		self.sync_focus_from_ui();

		// Get the document area (excluding panels/docks)
		let doc_area = dock_layout.doc_area;

		self.handle_mouse_in_doc_area(mouse, doc_area).await
	}

	/// Handles mouse events within the document area (where splits live).
	///
	/// This method:
	/// 1. Handles active separator drag (resize) operations
	/// 2. Checks if mouse is over a separator (for hover/resize feedback)
	/// 3. Determines which view the mouse is over
	/// 4. Focuses that view if it's different from the current focus
	/// 5. Translates screen coordinates to view-local coordinates
	/// 6. Dispatches the mouse event to the appropriate handler
	///
	/// Text selection drags are confined to the view where they started.
	/// This prevents selection from crossing split boundaries.
	pub(crate) async fn handle_mouse_in_doc_area(
		&mut self,
		mouse: termina::event::MouseEvent,
		doc_area: xeno_tui::layout::Rect,
	) -> bool {
		let mouse_x = mouse.column;
		let mouse_y = mouse.row;

		if let Some(drag_state) = self.layout.drag_state().cloned() {
			match mouse.kind {
				MouseEventKind::Drag(_) => {
					let base_layout = &mut self.windows.base_window_mut().layout;
					self.layout.resize_separator(
						base_layout,
						doc_area,
						&drag_state.id,
						mouse_x,
						mouse_y,
					);
					self.frame.needs_redraw = true;
					return false;
				}
				MouseEventKind::Up(_) => {
					self.layout.end_drag();
					self.frame.needs_redraw = true;
					return false;
				}
				_ => {}
			}
		}

		// Handle active text selection drag - confine to origin view
		if let Some((origin_view, origin_area)) = self.layout.text_selection_origin {
			match mouse.kind {
				MouseEventKind::Drag(_) => {
					let clamped_x =
						mouse_x.clamp(origin_area.x, origin_area.right().saturating_sub(1));
					let clamped_y =
						mouse_y.clamp(origin_area.y, origin_area.bottom().saturating_sub(1));
					let local_row = clamped_y.saturating_sub(origin_area.y);
					let local_col = clamped_x.saturating_sub(origin_area.x);

					let tab_width = self.tab_width_for(origin_view);
					if let Some(buffer) = self.buffers.get_buffer_mut(origin_view) {
						let _ = buffer.input.handle_mouse(mouse.into());
						let doc_pos = buffer
							.screen_to_doc_position(local_row, local_col, tab_width)
							.or_else(|| {
								let gutter_width = buffer.gutter_width();
								(local_col < gutter_width)
									.then(|| {
										buffer.screen_to_doc_position(
											local_row,
											gutter_width,
											tab_width,
										)
									})
									.flatten()
							});

						if let Some(doc_pos) = doc_pos {
							let anchor = buffer.selection.primary().anchor;
							buffer.set_selection(Selection::single(anchor, doc_pos));
							buffer.sync_cursor_to_selection();
						}
					}
					self.frame.needs_redraw = true;
					return false;
				}
				MouseEventKind::Up(_) => {
					self.layout.text_selection_origin = None;
					self.frame.needs_redraw = true;
				}
				_ => {}
			}
		}

		let mut floating_hit = None;
		for (window_id, window) in self.windows.floating_windows() {
			if window.contains(mouse_x, mouse_y) {
				floating_hit = Some((window_id, window));
			}
		}

		let separator_hit = if floating_hit.is_some() {
			None
		} else {
			let base_layout = &self.base_window().layout;
			self.layout
				.separator_hit_at_position(base_layout, doc_area, mouse_x, mouse_y)
		};

		self.layout.update_mouse_velocity(mouse_x, mouse_y);
		let is_fast_mouse = self.layout.is_mouse_fast();

		let current_separator = separator_hit.as_ref().map(|hit| (hit.direction, hit.rect));
		self.layout.separator_under_mouse = current_separator;

		match mouse.kind {
			MouseEventKind::Moved => {
				let old_hover = self.layout.hovered_separator;

				// Hover activation: sticky once active, velocity-gated for new hovers
				self.layout.hovered_separator = match (old_hover, current_separator) {
					(Some(old), Some(new)) if old == new => Some(old),
					(_, Some(sep)) if !is_fast_mouse => Some(sep),
					(_, Some(_)) => {
						self.frame.needs_redraw = true;
						None
					}
					(_, None) => None,
				};

				if old_hover != self.layout.hovered_separator {
					self.layout
						.update_hover_animation(old_hover, self.layout.hovered_separator);
					self.frame.needs_redraw = true;
				}

				if self.layout.hovered_separator.is_some() {
					return false;
				}
			}
			MouseEventKind::Down(_) => {
				if let Some(hit) = &separator_hit {
					self.layout.start_drag(hit);
					self.frame.needs_redraw = true;
					return false;
				}
				if self.layout.hovered_separator.is_some() {
					let old_hover = self.layout.hovered_separator.take();
					self.layout.update_hover_animation(old_hover, None);
					self.frame.needs_redraw = true;
				}
			}
			MouseEventKind::Drag(_) => {
				if self.layout.hovered_separator.is_some() {
					let old_hover = self.layout.hovered_separator.take();
					self.layout.update_hover_animation(old_hover, None);
					self.frame.needs_redraw = true;
				}
			}
			_ => {
				if separator_hit.is_none() && self.layout.hovered_separator.is_some() {
					let old_hover = self.layout.hovered_separator.take();
					self.layout.update_hover_animation(old_hover, None);
					self.frame.needs_redraw = true;
				}
			}
		}

		let view_hit = if let Some((window_id, window)) = floating_hit {
			Some((window.buffer, window.content_rect(), window_id))
		} else {
			let base_layout = &self.base_window().layout;
			self.layout
				.view_at_position(base_layout, doc_area, mouse_x, mouse_y)
				.map(|(view, area)| (view, area, self.windows.base_id()))
		};
		let Some((target_view, view_area, target_window)) = view_hit else {
			return false;
		};

		let focused_window = match &self.focus {
			FocusTarget::Buffer { window, .. } => Some(*window),
			FocusTarget::Panel(_) => None,
		};
		let sticky_floating = focused_window
			.and_then(|id| self.windows.get(id))
			.and_then(|window| match window {
				Window::Floating(floating) => Some(floating.sticky),
				Window::Base(_) => None,
			})
			.unwrap_or(false);

		if matches!(mouse.kind, MouseEventKind::Moved)
			&& sticky_floating
			&& Some(target_window) != focused_window
		{
			return false;
		}

		let needs_focus = match &self.focus {
			FocusTarget::Buffer { window, buffer } => {
				*window != target_window || *buffer != target_view
			}
			FocusTarget::Panel(_) => true,
		};
		if needs_focus {
			let focus_changed = match mouse.kind {
				MouseEventKind::Down(_) => {
					self.focus_buffer_in_window(target_window, target_view, true)
				}
				_ => {
					if target_window == self.windows.base_id() {
						self.focus_view_implicit(target_view)
					} else {
						self.focus_buffer_in_window(target_window, target_view, false)
					}
				}
			};
			if !focus_changed && needs_focus {
				return false;
			}
		}

		// Translate screen coordinates to view-local coordinates
		let local_row = mouse_y.saturating_sub(view_area.y);
		let local_col = mouse_x.saturating_sub(view_area.x);

		// Process the mouse event through the input handler
		let result = self.buffer_mut().input.handle_mouse(mouse.into());
		match result {
			KeyResult::MouseClick { extend, .. } => {
				self.layout.text_selection_origin = Some((target_view, view_area));
				self.handle_mouse_click_local(local_row, local_col, extend);
				false
			}
			KeyResult::MouseDrag { .. } => {
				self.handle_mouse_drag_local(local_row, local_col);
				false
			}
			KeyResult::MouseScroll { direction, count } => {
				self.handle_mouse_scroll(direction, count);
				false
			}
			_ => false,
		}
	}

	/// Returns the screen area of the currently focused view.
	///
	/// This computes the document area (excluding status line and panels)
	/// and then finds the focused view's rectangle within that area.
	pub(crate) fn focused_view_area(&self) -> xeno_tui::layout::Rect {
		let doc_area = self.doc_area();
		if let FocusTarget::Buffer { window, .. } = &self.focus
			&& *window != self.windows.base_id()
			&& let Some(Window::Floating(floating)) = self.windows.get(*window)
		{
			return floating.content_rect();
		}
		let focused = self.focused_view();
		for (view, area) in self
			.layout
			.compute_view_areas(&self.base_window().layout, doc_area)
		{
			if view == focused {
				return area;
			}
		}
		doc_area
	}

	/// Computes the document area based on current window dimensions.
	pub fn doc_area(&self) -> xeno_tui::layout::Rect {
		let width = self.viewport.width.unwrap_or(80);
		let height = self.viewport.height.unwrap_or(24);
		// Exclude status line (1 row)
		let main_height = height.saturating_sub(1);
		let main_area = xeno_tui::layout::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};
		self.ui.compute_layout(main_area).doc_area
	}
}
