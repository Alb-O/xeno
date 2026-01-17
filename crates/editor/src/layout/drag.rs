//! Drag state and mouse velocity tracking.
//!
//! Managing separator drag operations and hover animations.

use xeno_tui::layout::Rect;

use super::manager::LayoutManager;
use super::types::SeparatorHit;
use crate::buffer::SplitDirection;
use crate::separator::{DragState, SeparatorHoverAnimation};
use crate::test_events::{AnimationDirection, SeparatorAnimationEvent};

impl LayoutManager {
	/// Updates the mouse velocity tracker with a new position.
	pub fn update_mouse_velocity(&mut self, x: u16, y: u16) {
		self.mouse_velocity.update(x, y);
	}

	/// Returns true if the mouse is moving fast enough to suppress hover effects.
	pub fn is_mouse_fast(&self) -> bool {
		self.mouse_velocity.is_fast()
	}

	/// Starts a separator drag operation.
	pub fn start_drag(&mut self, hit: &SeparatorHit) {
		self.dragging_separator = Some(DragState {
			id: hit.id.clone(),
			revision: self.layout_revision(),
		});
		let old_hover = self.hovered_separator.take();
		self.hovered_separator = Some((hit.direction, hit.rect));
		if old_hover != self.hovered_separator {
			self.update_hover_animation(old_hover, self.hovered_separator);
		}
	}

	/// Checks if the current drag state is stale (layout changed since drag started).
	///
	/// Returns true if there is an active drag and the layout revision has changed
	/// since the drag started, meaning the stored separator path may be invalid.
	pub fn is_drag_stale(&self) -> bool {
		self.dragging_separator
			.as_ref()
			.map(|drag| drag.revision != self.layout_revision())
			.unwrap_or(false)
	}

	/// Cancels the drag if the layout has changed since it started.
	///
	/// Returns true if the drag was canceled due to staleness.
	pub fn cancel_if_stale(&mut self) -> bool {
		if self.is_drag_stale() {
			self.end_drag();
			true
		} else {
			false
		}
	}

	/// Ends the current separator drag operation.
	pub fn end_drag(&mut self) {
		self.dragging_separator = None;
		self.hovered_separator = None;
	}

	/// Returns true if a separator drag is active.
	pub fn is_dragging(&self) -> bool {
		self.dragging_separator.is_some()
	}

	/// Returns the current drag state, if any.
	pub fn drag_state(&self) -> Option<&DragState> {
		self.dragging_separator.as_ref()
	}

	/// Updates the separator hover animation when hover state changes.
	pub fn update_hover_animation(
		&mut self,
		old: Option<(SplitDirection, Rect)>,
		new: Option<(SplitDirection, Rect)>,
	) {
		match (old, new) {
			(None, Some((_, rect))) => {
				SeparatorAnimationEvent::start(AnimationDirection::FadeIn);
				self.separator_hover_animation = Some(SeparatorHoverAnimation::new(rect, true));
			}
			(Some((_, old_rect)), None) => {
				SeparatorAnimationEvent::start(AnimationDirection::FadeOut);
				if self
					.separator_hover_animation
					.as_ref()
					.is_some_and(|a| a.rect == old_rect)
				{
					self.separator_hover_animation
						.as_mut()
						.unwrap()
						.set_hovering(false);
				} else {
					self.separator_hover_animation =
						Some(SeparatorHoverAnimation::new_at_intensity(old_rect, 1.0, false));
				}
			}
			(Some((_, old_rect)), Some((_, new_rect))) if old_rect != new_rect => {
				SeparatorAnimationEvent::start(AnimationDirection::FadeIn);
				self.separator_hover_animation = Some(SeparatorHoverAnimation::new(new_rect, true));
			}
			_ => {}
		}
	}

	/// Returns true if the hover animation needs a redraw.
	pub fn animation_needs_redraw(&self) -> bool {
		self.separator_hover_animation
			.as_ref()
			.is_some_and(|a| a.needs_redraw())
	}

	/// Returns the animation intensity for the given separator rect.
	pub fn animation_intensity(&self) -> f32 {
		self.separator_hover_animation
			.as_ref()
			.map_or(0.0, |a| a.intensity())
	}

	/// Returns the rect being animated, if any.
	pub fn animation_rect(&self) -> Option<Rect> {
		self.separator_hover_animation.as_ref().map(|a| a.rect)
	}
}
