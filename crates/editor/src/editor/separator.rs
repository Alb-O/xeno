//! Separator hover and drag state for split resizing.

use xeno_tui::animation::{Easing, ToggleTween};
use xeno_tui::layout::Rect;

use super::layout::SeparatorId;

/// State for an active separator drag operation.
#[derive(Debug, Clone, PartialEq)]
pub struct DragState {
	/// Identifier of the separator being dragged.
	pub id: SeparatorId,
}

/// Tracks mouse velocity to determine if hover effects should be suppressed.
///
/// Fast mouse movement indicates the user is just passing through, not intending
/// to interact with separators. We suppress hover effects in this case to reduce
/// visual noise.
#[derive(Debug, Clone, Default)]
pub struct MouseVelocityTracker {
	/// Last known mouse position.
	last_position: Option<(u16, u16)>,
	/// When the last position was recorded.
	last_time: Option<std::time::Instant>,
	/// Smoothed velocity in cells per second.
	velocity: f32,
}

impl MouseVelocityTracker {
	/// Velocity threshold above which hover effects are suppressed (cells/second).
	const FAST_THRESHOLD: f32 = 60.0;

	/// Time after which velocity is considered zero (mouse is idle).
	const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100);

	/// Updates the tracker with a new mouse position and returns current velocity.
	pub fn update(&mut self, x: u16, y: u16) -> f32 {
		let now = std::time::Instant::now();

		if let (Some((lx, ly)), Some(lt)) = (self.last_position, self.last_time) {
			let dx = (x as f32 - lx as f32).abs();
			let dy = (y as f32 - ly as f32).abs();
			let distance = (dx * dx + dy * dy).sqrt();
			let dt = now.duration_since(lt).as_secs_f32();

			if dt > 0.0 && dt < 0.5 {
				// Ignore stale readings (> 500ms gap)
				let instant_velocity = distance / dt;
				// Exponential moving average for smoothing
				self.velocity = self.velocity * 0.6 + instant_velocity * 0.4;
			}
		}

		self.last_position = Some((x, y));
		self.last_time = Some(now);
		self.velocity
	}

	/// Returns true if the mouse is moving fast enough to suppress hover effects.
	///
	/// Accounts for idle time - if mouse hasn't moved recently, velocity is zero.
	pub fn is_fast(&self) -> bool {
		// If mouse has been idle, velocity is effectively zero
		if let Some(lt) = self.last_time
			&& lt.elapsed() > Self::IDLE_TIMEOUT
		{
			return false;
		}
		self.velocity > Self::FAST_THRESHOLD
	}

	/// Returns the current smoothed velocity, accounting for idle time.
	pub fn velocity(&self) -> f32 {
		if let Some(lt) = self.last_time
			&& lt.elapsed() > Self::IDLE_TIMEOUT
		{
			return 0.0;
		}
		self.velocity
	}
}

/// Animation state for separator hover effects.
///
/// Uses a `ToggleTween<f32>` internally for smooth fade in/out transitions.
#[derive(Debug, Clone)]
pub struct SeparatorHoverAnimation {
	/// The separator rectangle being animated.
	pub rect: Rect,
	/// The hover intensity tween (0.0 = unhovered, 1.0 = fully hovered).
	tween: ToggleTween<f32>,
}

impl SeparatorHoverAnimation {
	/// Duration of the hover fade animation.
	const FADE_DURATION: std::time::Duration = std::time::Duration::from_millis(120);

	/// Creates a new hover animation for the given separator.
	pub fn new(rect: Rect, hovering: bool) -> Self {
		let mut tween =
			ToggleTween::new(0.0f32, 1.0f32, Self::FADE_DURATION).with_easing(Easing::EaseOut);
		tween.set_active(hovering);
		Self { rect, tween }
	}

	/// Creates a new hover animation starting at a specific intensity.
	///
	/// This is useful for creating fade-out animations that should start
	/// from a fully hovered state (intensity 1.0).
	pub fn new_at_intensity(rect: Rect, intensity: f32, hovering: bool) -> Self {
		let tween = ToggleTween::new_at(0.0f32, 1.0f32, Self::FADE_DURATION, intensity, hovering)
			.with_easing(Easing::EaseOut);
		Self { rect, tween }
	}

	/// Returns whether we're animating toward hovered state.
	pub fn hovering(&self) -> bool {
		self.tween.is_active()
	}

	/// Sets the hover state, returning true if state changed.
	pub fn set_hovering(&mut self, hovering: bool) -> bool {
		self.tween.set_active(hovering)
	}

	/// Returns the effective hover intensity (0.0 = unhovered, 1.0 = fully hovered).
	pub fn intensity(&self) -> f32 {
		self.tween.value()
	}

	/// Returns true if the animation is complete.
	pub fn is_complete(&self) -> bool {
		self.tween.is_complete()
	}

	/// Returns true if the animation is still in progress.
	pub fn needs_redraw(&self) -> bool {
		self.tween.is_running()
	}
}
