use xeno_primitives::{MouseEvent, Selection};

use super::context::{MouseRouteContext, OverlayHit, ViewHit};
use super::routing::MouseRouteDecision;
use crate::buffer::{SplitDirection, ViewId};
use crate::geometry::Rect;
use crate::impls::{Editor, FocusReason, FocusTarget};
use crate::layout::SeparatorHit;
use crate::separator::DragState;

impl Editor {
	pub(super) fn apply_mouse_route(&mut self, context: MouseRouteContext, decision: MouseRouteDecision) -> bool {
		match decision {
			MouseRouteDecision::ContinueSeparatorDrag(drag_state) => self.apply_separator_drag_route(&context, drag_state),
			MouseRouteDecision::EndSeparatorDrag => self.apply_separator_release_route(),
			MouseRouteDecision::ContinueTextSelection { origin_view, origin_area } => self.apply_text_selection_drag_route(&context, origin_view, origin_area),
			MouseRouteDecision::OverlayPane(overlay_hit) => self.apply_overlay_route(&context, overlay_hit),
			MouseRouteDecision::Document { separator_hit, view_hit } => self.apply_document_route(&context, separator_hit, view_hit),
		}
	}

	fn apply_separator_drag_route(&mut self, context: &MouseRouteContext, drag_state: DragState) -> bool {
		if self.state.core.layout.cancel_if_stale() {
			self.state.core.frame.needs_redraw = true;
			return false;
		}

		let base_layout = &mut self.state.core.windows.base_window_mut().layout;
		self.state
			.core
			.layout
			.resize_separator(base_layout, context.doc_area, &drag_state.id, context.mouse_x, context.mouse_y);
		self.state.core.frame.needs_redraw = true;
		false
	}

	fn apply_separator_release_route(&mut self) -> bool {
		self.state.core.layout.end_drag();
		self.state.core.frame.needs_redraw = true;
		false
	}

	fn apply_text_selection_drag_route(&mut self, context: &MouseRouteContext, origin_view: ViewId, origin_area: Rect) -> bool {
		let clamped_x = context.mouse_x.clamp(origin_area.x, origin_area.right().saturating_sub(1));
		let clamped_y = context.mouse_y.clamp(origin_area.y, origin_area.bottom().saturating_sub(1));
		let local_row = clamped_y.saturating_sub(origin_area.y);
		let local_col = clamped_x.saturating_sub(origin_area.x);

		let tab_width = self.tab_width_for(origin_view);
		let scroll_lines = self.scroll_lines_for(origin_view);
		if let Some(buffer) = self.state.core.editor.buffers.get_buffer_mut(origin_view) {
			if let MouseEvent::Scroll { direction, .. } = context.mouse
				&& matches!(direction, xeno_primitives::ScrollDirection::Up | xeno_primitives::ScrollDirection::Down)
			{
				buffer.handle_mouse_scroll(direction, scroll_lines, tab_width);
			}

			let _ = buffer.input.handle_mouse(context.mouse);
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
		self.state.core.frame.needs_redraw = true;
		false
	}

	fn apply_overlay_route(&mut self, context: &MouseRouteContext, overlay_hit: OverlayHit) -> bool {
		self.maybe_clear_text_selection_origin_on_release(context);

		if overlay_hit.inner.width == 0 || overlay_hit.inner.height == 0 {
			return false;
		}

		let reason = if matches!(context.mouse, MouseEvent::Press { .. }) {
			FocusReason::Click
		} else {
			FocusReason::Programmatic
		};
		self.set_focus(FocusTarget::Overlay { buffer: overlay_hit.buffer }, reason);

		let clamped_x = context.mouse_x.clamp(overlay_hit.inner.x, overlay_hit.inner.right().saturating_sub(1));
		let clamped_y = context.mouse_y.clamp(overlay_hit.inner.y, overlay_hit.inner.bottom().saturating_sub(1));
		let local_row = clamped_y.saturating_sub(overlay_hit.inner.y);
		let local_col = clamped_x.saturating_sub(overlay_hit.inner.x);

		let result = self.buffer_mut().input.handle_mouse(context.mouse);
		let quit = self.apply_mouse_key_result(result, local_row, local_col, Some((overlay_hit.buffer, overlay_hit.inner)));
		self.state.core.frame.needs_redraw = true;
		quit
	}

	fn apply_document_route(&mut self, context: &MouseRouteContext, separator_hit: Option<SeparatorHit>, view_hit: Option<ViewHit>) -> bool {
		self.maybe_clear_text_selection_origin_on_release(context);

		self.state.core.layout.update_mouse_velocity(context.mouse_x, context.mouse_y);
		let is_fast_mouse = self.state.core.layout.is_mouse_fast();

		let current_separator = separator_hit.as_ref().map(|hit| (hit.direction, hit.rect));
		self.state.core.layout.separator_under_mouse = current_separator;

		if self.apply_separator_hover_and_drag_effects(context.mouse, separator_hit.as_ref(), current_separator, is_fast_mouse) {
			return false;
		}

		let Some(view_hit) = view_hit else {
			return false;
		};

		if !self.focus_view_for_mouse_event(context.mouse, view_hit) {
			return false;
		}

		let local_row = context.mouse_y.saturating_sub(view_hit.area.y);
		let local_col = context.mouse_x.saturating_sub(view_hit.area.x);

		let result = self.buffer_mut().input.handle_mouse(context.mouse);
		self.apply_mouse_key_result(result, local_row, local_col, Some((view_hit.view, view_hit.area)))
	}

	fn maybe_clear_text_selection_origin_on_release(&mut self, context: &MouseRouteContext) {
		if context.text_selection_origin.is_some() && matches!(context.mouse, MouseEvent::Release { .. }) {
			self.state.core.layout.text_selection_origin = None;
			self.state.core.frame.needs_redraw = true;
		}
	}

	fn apply_separator_hover_and_drag_effects(
		&mut self,
		mouse: MouseEvent,
		separator_hit: Option<&SeparatorHit>,
		current_separator: Option<(SplitDirection, Rect)>,
		is_fast_mouse: bool,
	) -> bool {
		match mouse {
			MouseEvent::Move { .. } => {
				let old_hover = self.state.core.layout.hovered_separator;

				// Hover activation: sticky once active, velocity-gated for new hovers
				self.state.core.layout.hovered_separator = match (old_hover, current_separator) {
					(Some(old), Some(new)) if old == new => Some(old),
					(_, Some(sep)) if !is_fast_mouse => Some(sep),
					(_, Some(_)) => {
						self.state.core.frame.needs_redraw = true;
						None
					}
					(_, None) => None,
				};

				if old_hover != self.state.core.layout.hovered_separator {
					self.state
						.core
						.layout
						.update_hover_animation(old_hover, self.state.core.layout.hovered_separator);
					self.state.core.frame.needs_redraw = true;
				}

				self.state.core.layout.hovered_separator.is_some()
			}
			MouseEvent::Press { .. } => {
				if let Some(hit) = separator_hit {
					self.state.core.layout.start_drag(hit);
					self.state.core.frame.needs_redraw = true;
					return true;
				}
				if self.state.core.layout.hovered_separator.is_some() {
					let old_hover = self.state.core.layout.hovered_separator.take();
					self.state.core.layout.update_hover_animation(old_hover, None);
					self.state.core.frame.needs_redraw = true;
				}
				false
			}
			MouseEvent::Drag { .. } => {
				if self.state.core.layout.hovered_separator.is_some() {
					let old_hover = self.state.core.layout.hovered_separator.take();
					self.state.core.layout.update_hover_animation(old_hover, None);
					self.state.core.frame.needs_redraw = true;
				}
				false
			}
			_ => {
				if separator_hit.is_none() && self.state.core.layout.hovered_separator.is_some() {
					let old_hover = self.state.core.layout.hovered_separator.take();
					self.state.core.layout.update_hover_animation(old_hover, None);
					self.state.core.frame.needs_redraw = true;
				}
				false
			}
		}
	}

	fn focus_view_for_mouse_event(&mut self, mouse: MouseEvent, view_hit: ViewHit) -> bool {
		let needs_focus = match &self.state.core.focus {
			FocusTarget::Buffer { window, buffer } => *window != view_hit.window || *buffer != view_hit.view,
			FocusTarget::Overlay { .. } => true,
			FocusTarget::Panel(_) => true,
		};
		if !needs_focus {
			return true;
		}

		let focus_reason = match mouse {
			MouseEvent::Press { .. } => FocusReason::Click,
			_ if view_hit.window == self.state.core.windows.base_id() => FocusReason::Hover,
			_ => FocusReason::Programmatic,
		};
		self.set_focus(
			FocusTarget::Buffer {
				window: view_hit.window,
				buffer: view_hit.view,
			},
			focus_reason,
		)
	}
}
