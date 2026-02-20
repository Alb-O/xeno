//! Separator hover and drag state for split resizing.

use std::time::{Duration, Instant};

use crate::geometry::Rect;
use crate::layout::SeparatorId;

/// State for an active separator drag operation.
#[derive(Debug, Clone, PartialEq)]
pub struct DragState {
	/// Identifier of the separator being dragged.
	pub id: SeparatorId,
	/// Structural layout revision when the drag started.
	///
	/// If the layout changes during a drag (e.g., view closed via keybinding),
	/// the stored path may become invalid. Comparing this against the current
	/// structure revision allows detecting and canceling stale drags.
	pub structure_revision: u64,
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
	///
	/// Uses exponential moving average for smoothing. Readings with gaps over 500ms
	/// are ignored to avoid velocity spikes from stale data.
	pub fn update(&mut self, x: u16, y: u16) -> f32 {
		let now = std::time::Instant::now();

		if let (Some((lx, ly)), Some(lt)) = (self.last_position, self.last_time) {
			let dx = (x as f32 - lx as f32).abs();
			let dy = (y as f32 - ly as f32).abs();
			let distance = (dx * dx + dy * dy).sqrt();
			let dt = now.duration_since(lt).as_secs_f32();

			if dt > 0.0 && dt < 0.5 {
				let instant_velocity = distance / dt;
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
		self.last_time.is_some_and(|lt| lt.elapsed() <= Self::IDLE_TIMEOUT) && self.velocity > Self::FAST_THRESHOLD
	}

	/// Returns the current smoothed velocity, accounting for idle time.
	pub fn velocity(&self) -> f32 {
		if self.last_time.is_some_and(|lt| lt.elapsed() > Self::IDLE_TIMEOUT) {
			0.0
		} else {
			self.velocity
		}
	}
}

/// Animation state for separator hover effects.
///
/// Uses a lightweight time-based tween for smooth fade in/out transitions.
#[derive(Debug, Clone)]
pub struct SeparatorHoverAnimation {
	/// The separator rectangle being animated.
	pub rect: Rect,
	/// Whether the target state is hovered.
	active: bool,
	/// Intensity at animation start.
	start_value: f32,
	/// Target intensity (0.0 or 1.0).
	target_value: f32,
	/// Animation start time.
	started_at: Instant,
}

impl SeparatorHoverAnimation {
	/// Duration of the hover fade animation.
	const FADE_DURATION: Duration = Duration::from_millis(120);

	fn target_for(hovering: bool) -> f32 {
		if hovering { 1.0 } else { 0.0 }
	}

	fn ease_out(progress: f32) -> f32 {
		let p = progress.clamp(0.0, 1.0);
		1.0 - (1.0 - p) * (1.0 - p)
	}

	fn current_value(&self) -> f32 {
		let elapsed = self.started_at.elapsed();
		if elapsed >= Self::FADE_DURATION {
			return self.target_value;
		}

		let progress = elapsed.as_secs_f32() / Self::FADE_DURATION.as_secs_f32();
		let eased = Self::ease_out(progress);
		self.start_value + (self.target_value - self.start_value) * eased
	}

	/// Creates a new hover animation for the given separator.
	pub fn new(rect: Rect, hovering: bool) -> Self {
		Self {
			rect,
			active: hovering,
			start_value: if hovering { 0.0 } else { Self::target_for(hovering) },
			target_value: Self::target_for(hovering),
			started_at: Instant::now(),
		}
	}

	/// Creates a new hover animation starting at a specific intensity.
	///
	/// This is useful for creating fade-out animations that should start
	/// from a fully hovered state (intensity 1.0).
	pub fn new_at_intensity(rect: Rect, intensity: f32, hovering: bool) -> Self {
		Self {
			rect,
			active: hovering,
			start_value: intensity.clamp(0.0, 1.0),
			target_value: Self::target_for(hovering),
			started_at: Instant::now(),
		}
	}

	/// Returns whether we're animating toward hovered state.
	pub fn hovering(&self) -> bool {
		self.active
	}

	/// Sets the hover state, returning true if state changed.
	pub fn set_hovering(&mut self, hovering: bool) -> bool {
		if self.active == hovering {
			return false;
		}
		self.start_value = self.current_value();
		self.target_value = Self::target_for(hovering);
		self.active = hovering;
		self.started_at = Instant::now();
		true
	}

	/// Returns the effective hover intensity (0.0 = unhovered, 1.0 = fully hovered).
	pub fn intensity(&self) -> f32 {
		self.current_value()
	}

	/// Returns true if the animation is complete.
	pub fn is_complete(&self) -> bool {
		self.started_at.elapsed() >= Self::FADE_DURATION
	}

	/// Returns true if the animation is still in progress.
	pub fn needs_redraw(&self) -> bool {
		!self.is_complete()
	}
}
