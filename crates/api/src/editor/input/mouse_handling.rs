//! Mouse event handling.
//!
//! Processing mouse input for text selection, separator dragging, and panels.

use evildoer_base::Selection;
use evildoer_input::KeyResult;
use termina::event::MouseEventKind;

use super::conversions::convert_mouse_event;
use crate::buffer::BufferView;
use crate::editor::Editor;

impl Editor {
	pub async fn handle_mouse(&mut self, mouse: termina::event::MouseEvent) -> bool {
		let width = self.window_width.unwrap_or(80);
		let height = self.window_height.unwrap_or(24);
		// Main area excludes status line (1 row)
		let main_height = height.saturating_sub(1);
		let main_area = evildoer_tui::layout::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};

		let mut ui = std::mem::take(&mut self.ui);
		let dock_layout = ui.compute_layout(main_area);

		if ui.handle_mouse(self, mouse, &dock_layout) {
			if ui.take_wants_redraw() {
				self.needs_redraw = true;
			}
			self.ui = ui;
			return false;
		}
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;

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
		doc_area: evildoer_tui::layout::Rect,
	) -> bool {
		let mouse_x = mouse.column;
		let mouse_y = mouse.row;

		if let Some(drag_state) = self.layout.drag_state().cloned() {
			match mouse.kind {
				MouseEventKind::Drag(_) => {
					self.layout
						.resize_separator(doc_area, &drag_state.id, mouse_x, mouse_y);
					self.needs_redraw = true;
					return false;
				}
				MouseEventKind::Up(_) => {
					self.layout.end_drag();
					self.needs_redraw = true;
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

					if let BufferView::Text(buffer_id) = origin_view
						&& let Some(buffer) = self.buffers.get_buffer_mut(buffer_id)
					{
						let _ = buffer.input.handle_mouse(mouse.into());
						let doc_pos =
							buffer
								.screen_to_doc_position(local_row, local_col)
								.or_else(|| {
									let gutter_width = buffer.gutter_width();
									(local_col < gutter_width)
										.then(|| {
											buffer.screen_to_doc_position(local_row, gutter_width)
										})
										.flatten()
								});

						if let Some(doc_pos) = doc_pos {
							let anchor = buffer.selection.primary().anchor;
							buffer.selection = Selection::single(anchor, doc_pos);
							buffer.cursor = buffer.selection.primary().head;
						}
					}
					self.needs_redraw = true;
					return false;
				}
				MouseEventKind::Up(_) => {
					self.layout.text_selection_origin = None;
					self.needs_redraw = true;
				}
				_ => {}
			}
		}

		let separator_hit = self
			.layout
			.separator_hit_at_position(doc_area, mouse_x, mouse_y);

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
						self.needs_redraw = true;
						None
					}
					(_, None) => None,
				};

				if old_hover != self.layout.hovered_separator {
					self.layout
						.update_hover_animation(old_hover, self.layout.hovered_separator);
					self.needs_redraw = true;
				}

				if self.layout.hovered_separator.is_some() {
					return false;
				}
			}
			MouseEventKind::Down(_) => {
				if let Some(hit) = &separator_hit {
					self.layout.start_drag(hit);
					self.needs_redraw = true;
					return false;
				}
				if self.layout.hovered_separator.is_some() {
					let old_hover = self.layout.hovered_separator.take();
					self.layout.update_hover_animation(old_hover, None);
					self.needs_redraw = true;
				}
			}
			MouseEventKind::Drag(_) => {
				if self.layout.hovered_separator.is_some() {
					let old_hover = self.layout.hovered_separator.take();
					self.layout.update_hover_animation(old_hover, None);
					self.needs_redraw = true;
				}
			}
			_ => {
				if separator_hit.is_none() && self.layout.hovered_separator.is_some() {
					let old_hover = self.layout.hovered_separator.take();
					self.layout.update_hover_animation(old_hover, None);
					self.needs_redraw = true;
				}
			}
		}

		let Some((target_view, view_area)) =
			self.layout.view_at_position(doc_area, mouse_x, mouse_y)
		else {
			return false;
		};

		if target_view != self.focused_view() {
			let focus_changed = match mouse.kind {
				MouseEventKind::Down(_) => self.focus_view(target_view),
				_ => self.focus_view_implicit(target_view),
			};
			if !focus_changed && target_view != self.focused_view() {
				return false;
			}
		}

		if let BufferView::Panel(panel_id) = self.focused_view() {
			let local_x = mouse_x.saturating_sub(view_area.x);
			let local_y = mouse_y.saturating_sub(view_area.y);

			if let Some(split_mouse) = convert_mouse_event(&mouse, local_x, local_y) {
				let result = self.handle_panel_mouse(panel_id, split_mouse);
				if result.needs_redraw {
					self.needs_redraw = true;
				}
			}
			return false;
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
	pub(crate) fn focused_view_area(&self) -> evildoer_tui::layout::Rect {
		let doc_area = self.doc_area();
		let focused = self.focused_view();
		for (view, area) in self.layout.compute_view_areas(doc_area) {
			if view == focused {
				return area;
			}
		}
		doc_area
	}

	/// Computes the document area based on current window dimensions.
	pub fn doc_area(&self) -> evildoer_tui::layout::Rect {
		let width = self.window_width.unwrap_or(80);
		let height = self.window_height.unwrap_or(24);
		let main_height = height.saturating_sub(1);
		let main_area = evildoer_tui::layout::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};
		self.ui.compute_layout(main_area).doc_area
	}
}
