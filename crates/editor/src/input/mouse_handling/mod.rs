//! Mouse event handling.
//!
//! Processing mouse input for text selection and separator dragging.

mod context;
mod effects;
mod routing;

use routing::decide_mouse_route;
use xeno_input::input::KeyResult;
use xeno_primitives::MouseEvent;

use crate::impls::{Editor, FocusTarget};

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

		let quit = self.handle_mouse_in_doc_area(mouse, doc_area).await;
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
		let context = self.build_mouse_route_context(mouse, doc_area);
		let decision = decide_mouse_route(&context);
		self.apply_mouse_route(context, decision)
	}

	fn apply_mouse_key_result(
		&mut self,
		result: KeyResult,
		local_row: u16,
		local_col: u16,
		selection_origin: Option<(crate::buffer::ViewId, crate::geometry::Rect)>,
	) -> bool {
		match result {
			KeyResult::MouseClick { extend, .. } => {
				self.state.layout.text_selection_origin = selection_origin;
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
