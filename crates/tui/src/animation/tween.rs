//! Time-based animation tweening.

use std::time::{Duration, Instant};

use crate::animation::easing::Easing;
use crate::animation::lerp::Animatable;

/// A time-based tween that animates a value from `start` to `end`.
///
/// Tracks animation progress based on elapsed time and applies easing
/// for smooth, natural motion.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use tome_tui::animation::{Animatable, Easing, Tween};
///
/// // Animate from 0.0 to 100.0 over 500ms with ease-out
/// let tween = Tween::new(0.0f32, 100.0f32, Duration::from_millis(500))
///     .with_easing(Easing::EaseOut);
///
/// // Immediately after creation, value is very close to start
/// assert!(tween.value() < 1.0);
///
/// // Check if animation is still running
/// if !tween.is_complete() {
///     // Animation in progress
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Tween<T: Animatable> {
	/// Starting value.
	pub start: T,
	/// Target value.
	pub end: T,
	/// When the animation started.
	pub start_time: Instant,
	/// Total animation duration.
	pub duration: Duration,
	/// Easing function to apply.
	pub easing: Easing,
}

impl<T: Animatable> Tween<T> {
	/// Creates a new tween with linear easing.
	///
	/// The animation starts immediately (from the current instant).
	pub fn new(start: T, end: T, duration: Duration) -> Self {
		Self {
			start,
			end,
			start_time: Instant::now(),
			duration,
			easing: Easing::Linear,
		}
	}

	/// Creates a new tween starting at a specific instant.
	pub fn new_at(start: T, end: T, duration: Duration, start_time: Instant) -> Self {
		Self {
			start,
			end,
			start_time,
			duration,
			easing: Easing::Linear,
		}
	}

	/// Sets the easing function (builder pattern).
	#[must_use]
	pub fn with_easing(mut self, easing: Easing) -> Self {
		self.easing = easing;
		self
	}

	/// Returns linear progress (0.0 to 1.0) based on elapsed time.
	///
	/// Does not apply easing.
	#[inline]
	pub fn progress(&self) -> f32 {
		if self.duration.is_zero() {
			return 1.0;
		}
		let elapsed = self.start_time.elapsed().as_secs_f32();
		let duration = self.duration.as_secs_f32();
		(elapsed / duration).min(1.0)
	}

	/// Returns eased progress (0.0 to 1.0).
	///
	/// Applies the configured easing function to linear progress.
	#[inline]
	pub fn eased_progress(&self) -> f32 {
		self.easing.apply(self.progress())
	}

	/// Returns the current interpolated value.
	///
	/// Applies easing and lerps between start and end.
	#[inline]
	pub fn value(&self) -> T {
		self.start.lerp(&self.end, self.eased_progress())
	}

	/// Returns true if the animation has completed.
	#[inline]
	pub fn is_complete(&self) -> bool {
		self.progress() >= 1.0
	}

	/// Returns true if the animation is still in progress.
	#[inline]
	pub fn is_running(&self) -> bool {
		!self.is_complete()
	}

	/// Returns remaining time until completion.
	pub fn remaining(&self) -> Duration {
		self.duration.saturating_sub(self.start_time.elapsed())
	}

	/// Resets the animation to start from now.
	pub fn reset(&mut self) {
		self.start_time = Instant::now();
	}

	/// Reverses the animation direction.
	///
	/// Swaps start and end values and resets the timer.
	pub fn reverse(&mut self) {
		std::mem::swap(&mut self.start, &mut self.end);
		self.reset();
	}

	/// Creates a reversed copy of this tween.
	///
	/// Swaps start and end, starting from now.
	#[must_use]
	pub fn reversed(&self) -> Self {
		Self::new(self.end.clone(), self.start.clone(), self.duration).with_easing(self.easing)
	}

	/// Retargets the animation to a new end value.
	///
	/// Starts from the current value, preserving momentum.
	pub fn retarget(&mut self, new_end: T) {
		self.start = self.value();
		self.end = new_end;
		self.reset();
	}
}

/// A reversible tween that can animate back and forth.
///
/// Useful for hover effects and toggleable states where you want
/// smooth transitions in both directions.
///
/// # Example
///
/// ```
/// use std::time::Duration;
/// use tome_tui::animation::{Easing, ToggleTween};
///
/// let mut toggle = ToggleTween::new(0.0f32, 1.0f32, Duration::from_millis(200))
///     .with_easing(Easing::EaseOut);
///
/// // Initial state: inactive (at start value)
/// assert_eq!(toggle.value(), 0.0);
/// assert!(!toggle.is_active());
///
/// // Activate: animates toward end value
/// toggle.set_active(true);
/// assert!(toggle.is_active());
/// ```
#[derive(Debug, Clone)]
pub struct ToggleTween<T: Animatable> {
	/// The underlying tween.
	tween: Tween<T>,
	/// Whether the toggle is currently active (animating toward end).
	active: bool,
	/// The "off" value.
	off_value: T,
	/// The "on" value.
	on_value: T,
}

impl<T: Animatable> ToggleTween<T> {
	/// Creates a new toggle tween, initially inactive (starting at `off` value).
	pub fn new(off: T, on: T, duration: Duration) -> Self {
		Self {
			tween: Tween::new(off.clone(), off.clone(), duration),
			active: false,
			off_value: off,
			on_value: on,
		}
	}

	/// Creates a new toggle tween starting at a specific value.
	///
	/// This is useful when you need to create an animation that starts from
	/// a known position (e.g., creating a fade-out animation that should
	/// start from fully visible).
	pub fn new_at(off: T, on: T, duration: Duration, start_value: T, active: bool) -> Self {
		let target = if active { on.clone() } else { off.clone() };
		Self {
			tween: Tween::new(start_value, target, duration),
			active,
			off_value: off,
			on_value: on,
		}
	}

	/// Sets the easing function (builder pattern).
	#[must_use]
	pub fn with_easing(mut self, easing: Easing) -> Self {
		self.tween.easing = easing;
		self
	}

	/// Returns whether the toggle is active.
	pub fn is_active(&self) -> bool {
		self.active
	}

	/// Sets the active state, triggering animation if changed.
	///
	/// Returns true if the state changed.
	pub fn set_active(&mut self, active: bool) -> bool {
		if self.active == active {
			return false;
		}
		self.active = active;

		// Start from current position
		let current = self.value();
		if active {
			self.tween = Tween::new(current, self.on_value.clone(), self.tween.duration)
				.with_easing(self.tween.easing);
		} else {
			self.tween = Tween::new(current, self.off_value.clone(), self.tween.duration)
				.with_easing(self.tween.easing);
		}
		true
	}

	/// Toggles the active state.
	pub fn toggle(&mut self) {
		self.set_active(!self.active);
	}

	/// Returns the current interpolated value.
	#[inline]
	pub fn value(&self) -> T {
		self.tween.value()
	}

	/// Returns linear progress (0.0 to 1.0).
	#[inline]
	pub fn progress(&self) -> f32 {
		self.tween.progress()
	}

	/// Returns true if the animation is complete.
	#[inline]
	pub fn is_complete(&self) -> bool {
		self.tween.is_complete()
	}

	/// Returns true if animation is in progress.
	#[inline]
	pub fn is_running(&self) -> bool {
		self.tween.is_running()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_tween_immediate_value() {
		let tween = Tween::new(0.0f32, 100.0f32, Duration::from_millis(100));
		// Immediately after creation, value should be very close to start
		// (allowing for tiny elapsed time between creation and check)
		assert!(
			tween.value() < 1.0,
			"expected near-zero, got {}",
			tween.value()
		);
	}

	#[test]
	fn test_tween_zero_duration() {
		let tween = Tween::new(0.0f32, 100.0f32, Duration::ZERO);
		assert!(tween.is_complete());
		assert_eq!(tween.value(), 100.0);
	}

	#[test]
	fn test_tween_reversed() {
		let tween = Tween::new(0.0f32, 100.0f32, Duration::from_millis(100));
		let reversed = tween.reversed();
		assert_eq!(reversed.start, 100.0);
		assert_eq!(reversed.end, 0.0);
	}

	#[test]
	fn test_toggle_initial_state() {
		let toggle = ToggleTween::new(0.0f32, 1.0f32, Duration::from_millis(100));
		assert!(!toggle.is_active());
		assert_eq!(toggle.value(), 0.0);
	}

	#[test]
	fn test_toggle_activation() {
		let mut toggle = ToggleTween::new(0.0f32, 1.0f32, Duration::from_millis(100));
		let changed = toggle.set_active(true);
		assert!(changed);
		assert!(toggle.is_active());

		// Setting to same state should return false
		let changed = toggle.set_active(true);
		assert!(!changed);
	}
}
