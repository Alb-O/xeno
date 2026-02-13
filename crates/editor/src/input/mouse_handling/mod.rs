//! Mouse event handling.
//!
//! Processing mouse input for text selection and separator dragging.

use xeno_input::input::KeyResult;
use xeno_primitives::{MouseEvent, ScrollDirection, Selection};

use crate::impls::{Editor, FocusReason, FocusTarget};

impl Editor {
	/// Processes a mouse event, returning true if the event triggered a quit.
	pub async fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
		let width = self.state.viewport.width.unwrap_or(80);
		let height = self.state.viewport.height.unwrap_or(24);

		// Main area excludes status line (1 row)
		let main_height = height.saturating_sub(1);
		let main_area = crate::geometry::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};

		let mut ui = std::mem::take(&mut self.state.ui);
		let dock_layout = ui.compute_layout(main_area);

		let hit_is_panel = dock_layout.panel_areas.values().any(|area| {
			mouse.col() >= area.x
				&& mouse.col() < area.x.saturating_add(area.width)
				&& mouse.row() >= area.y
				&& mouse.row() < area.y.saturating_add(area.height)
		});

		let hit_active_overlay = self.state.overlay_system.interaction().active().is_some_and(|active| {
			active.session.panes.iter().any(|pane| {
				mouse.col() >= pane.rect.x
					&& mouse.col() < pane.rect.x.saturating_add(pane.rect.width)
					&& mouse.row() >= pane.rect.y
					&& mouse.row() < pane.rect.y.saturating_add(pane.rect.height)
			})
		});

		if hit_is_panel && !hit_active_overlay {
			if ui.handle_mouse(self, mouse, &dock_layout) {
				if ui.take_wants_redraw() {
					self.state.frame.needs_redraw = true;
				}
				self.state.ui = ui;
				self.sync_focus_from_ui();
				self.interaction_on_buffer_edited();
				return false;
			}
		} else if ui.focused_panel_id().is_some() {
			ui.apply_requests(vec![crate::ui::UiRequest::Focus(crate::ui::UiFocus::editor())]);
		}
		if ui.take_wants_redraw() {
			self.state.frame.needs_redraw = true;
		}
		self.state.ui = ui;
		self.sync_focus_from_ui();

		// Get the document area (excluding panels/docks)
		let doc_area = dock_layout.doc_area;

		let quit = self.handle_mouse_in_doc_area(mouse, doc_area.into()).await;
		self.interaction_on_buffer_edited();
		quit
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
	pub(crate) async fn handle_mouse_in_doc_area(&mut self, mouse: MouseEvent, doc_area: crate::geometry::Rect) -> bool {
		let mouse_x = mouse.col();
		let mouse_y = mouse.row();

		if let Some(drag_state) = self.state.layout.drag_state().cloned() {
			match mouse {
				MouseEvent::Drag { .. } => {
					let base_layout = &mut self.state.windows.base_window_mut().layout;
					self.state.layout.resize_separator(base_layout, doc_area, &drag_state.id, mouse_x, mouse_y);
					self.state.frame.needs_redraw = true;
					return false;
				}
				MouseEvent::Release { .. } => {
					self.state.layout.end_drag();
					self.state.frame.needs_redraw = true;
					return false;
				}
				_ => {}
			}
		}

		// Handle active text selection drag - confine to origin view
		if let Some((origin_view, origin_area)) = self.state.layout.text_selection_origin {
			match mouse {
				MouseEvent::Drag { .. }
				| MouseEvent::Scroll {
					direction: ScrollDirection::Up | ScrollDirection::Down,
					..
				} => {
					let clamped_x = mouse_x.clamp(origin_area.x, origin_area.right().saturating_sub(1));
					let clamped_y = mouse_y.clamp(origin_area.y, origin_area.bottom().saturating_sub(1));
					let local_row = clamped_y.saturating_sub(origin_area.y);
					let local_col = clamped_x.saturating_sub(origin_area.x);

					let tab_width = self.tab_width_for(origin_view);
					let scroll_lines = self.scroll_lines_for(origin_view);
					if let Some(buffer) = self.state.core.buffers.get_buffer_mut(origin_view) {
						if let MouseEvent::Scroll { direction, .. } = mouse
							&& matches!(direction, ScrollDirection::Up | ScrollDirection::Down)
						{
							buffer.handle_mouse_scroll(direction, scroll_lines, tab_width);
						}

						let _ = buffer.input.handle_mouse(mouse);
						let doc_pos = buffer.screen_to_doc_position(local_row, local_col, tab_width).or_else(|| {
							let gutter_width = buffer.gutter_width();
							(local_col < gutter_width)
								.then(|| buffer.screen_to_doc_position(local_row, gutter_width, tab_width))
								.flatten()
						});

						if let Some(doc_pos) = doc_pos {
							let anchor = buffer.selection.primary().anchor;
							buffer.set_selection(Selection::single(anchor, doc_pos));
							buffer.sync_cursor_to_selection();
						}
					}
					self.state.frame.needs_redraw = true;
					return false;
				}
				MouseEvent::Release { .. } => {
					self.state.layout.text_selection_origin = None;
					self.state.frame.needs_redraw = true;
				}
				_ => {}
			}
		}

		let overlay_hit = self.state.overlay_system.interaction().active().and_then(|active| {
			active
				.session
				.panes
				.iter()
				.rev()
				.find(|pane| {
					mouse_x >= pane.rect.x
						&& mouse_x < pane.rect.x.saturating_add(pane.rect.width)
						&& mouse_y >= pane.rect.y
						&& mouse_y < pane.rect.y.saturating_add(pane.rect.height)
				})
				.map(|pane| (pane.buffer, pane.rect, pane.style.clone()))
		});

		if let Some((overlay_buffer, overlay_rect, overlay_style)) = overlay_hit {
			let inner = crate::overlay::geom::pane_inner_rect(overlay_rect, &overlay_style);
			if inner.width == 0 || inner.height == 0 {
				return false;
			}

			let reason = if matches!(mouse, MouseEvent::Press { .. }) {
				FocusReason::Click
			} else {
				FocusReason::Programmatic
			};
			self.set_focus(FocusTarget::Overlay { buffer: overlay_buffer }, reason);

			let clamped_x = mouse_x.clamp(inner.x, inner.right().saturating_sub(1));
			let clamped_y = mouse_y.clamp(inner.y, inner.bottom().saturating_sub(1));
			let local_row = clamped_y.saturating_sub(inner.y);
			let local_col = clamped_x.saturating_sub(inner.x);

			let result = self.buffer_mut().input.handle_mouse(mouse);
			match result {
				KeyResult::MouseClick { extend, .. } => {
					self.state.layout.text_selection_origin = Some((overlay_buffer, inner));
					self.handle_mouse_click_local(local_row, local_col, extend);
				}
				KeyResult::MouseDrag { .. } => {
					self.handle_mouse_drag_local(local_row, local_col);
				}
				KeyResult::MouseScroll { direction, count } => {
					self.handle_mouse_scroll(direction, count);
				}
				_ => {}
			}

			self.state.frame.needs_redraw = true;
			return false;
		}

		let separator_hit = {
			let base_layout = &self.base_window().layout;
			self.state.layout.separator_hit_at_position(base_layout, doc_area, mouse_x, mouse_y)
		};

		self.state.layout.update_mouse_velocity(mouse_x, mouse_y);
		let is_fast_mouse = self.state.layout.is_mouse_fast();

		let current_separator = separator_hit.as_ref().map(|hit| (hit.direction, hit.rect));
		self.state.layout.separator_under_mouse = current_separator;

		match mouse {
			MouseEvent::Move { .. } => {
				let old_hover = self.state.layout.hovered_separator;

				// Hover activation: sticky once active, velocity-gated for new hovers
				self.state.layout.hovered_separator = match (old_hover, current_separator) {
					(Some(old), Some(new)) if old == new => Some(old),
					(_, Some(sep)) if !is_fast_mouse => Some(sep),
					(_, Some(_)) => {
						self.state.frame.needs_redraw = true;
						None
					}
					(_, None) => None,
				};

				if old_hover != self.state.layout.hovered_separator {
					self.state.layout.update_hover_animation(old_hover, self.state.layout.hovered_separator);
					self.state.frame.needs_redraw = true;
				}

				if self.state.layout.hovered_separator.is_some() {
					return false;
				}
			}
			MouseEvent::Press { .. } => {
				if let Some(hit) = &separator_hit {
					self.state.layout.start_drag(hit);
					self.state.frame.needs_redraw = true;
					return false;
				}
				if self.state.layout.hovered_separator.is_some() {
					let old_hover = self.state.layout.hovered_separator.take();
					self.state.layout.update_hover_animation(old_hover, None);
					self.state.frame.needs_redraw = true;
				}
			}
			MouseEvent::Drag { .. } => {
				if self.state.layout.hovered_separator.is_some() {
					let old_hover = self.state.layout.hovered_separator.take();
					self.state.layout.update_hover_animation(old_hover, None);
					self.state.frame.needs_redraw = true;
				}
			}
			_ => {
				if separator_hit.is_none() && self.state.layout.hovered_separator.is_some() {
					let old_hover = self.state.layout.hovered_separator.take();
					self.state.layout.update_hover_animation(old_hover, None);
					self.state.frame.needs_redraw = true;
				}
			}
		}

		let view_hit = {
			let base_layout = &self.base_window().layout;
			self.state
				.layout
				.view_at_position(base_layout, doc_area, mouse_x, mouse_y)
				.map(|(view, area)| (view, area, self.state.windows.base_id()))
		};
		let Some((target_view, view_area, target_window)) = view_hit else {
			return false;
		};

		let needs_focus = match &self.state.focus {
			FocusTarget::Buffer { window, buffer } => *window != target_window || *buffer != target_view,
			FocusTarget::Overlay { .. } => true,
			FocusTarget::Panel(_) => true,
		};
		if needs_focus {
			let focus_reason = match mouse {
				MouseEvent::Press { .. } => FocusReason::Click,
				_ if target_window == self.state.windows.base_id() => FocusReason::Hover,
				_ => FocusReason::Programmatic,
			};
			let focus_changed = self.set_focus(
				FocusTarget::Buffer {
					window: target_window,
					buffer: target_view,
				},
				focus_reason,
			);
			if !focus_changed && needs_focus {
				return false;
			}
		}

		// Translate screen coordinates to view-local coordinates
		let local_row = mouse_y.saturating_sub(view_area.y);
		let local_col = mouse_x.saturating_sub(view_area.x);

		// Process the mouse event through the input handler
		let result = self.buffer_mut().input.handle_mouse(mouse);
		match result {
			KeyResult::MouseClick { extend, .. } => {
				self.state.layout.text_selection_origin = Some((target_view, view_area));
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
	pub(crate) fn focused_view_area(&self) -> crate::geometry::Rect {
		let doc_area = self.doc_area();
		if let FocusTarget::Overlay { buffer } = &self.state.focus {
			return self.view_area(*buffer);
		}
		let focused = self.focused_view();
		for (view, area) in self.state.layout.compute_view_areas(&self.base_window().layout, doc_area) {
			if view == focused {
				return area;
			}
		}
		doc_area
	}

	/// Computes the document area based on current window dimensions.
	pub fn doc_area(&self) -> crate::geometry::Rect {
		let width = self.state.viewport.width.unwrap_or(80);
		let height = self.state.viewport.height.unwrap_or(24);
		// Exclude status line (1 row)
		let main_height = height.saturating_sub(1);
		let main_area = crate::geometry::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};
		self.state.ui.compute_layout(main_area).doc_area
	}
}

#[cfg(test)]
mod tests;
