//! Easing functions for animation curves.
//!
//! Easing functions transform linear progress (0.0 to 1.0) into curved
//! progress, creating more natural-feeling animations.

/// Easing function for controlling animation curves.
///
/// Transforms linear progress `t ∈ [0.0, 1.0]` into curved progress,
/// creating more natural motion.
///
/// # Variants
///
/// - **Linear**: Constant speed, no acceleration.
/// - **EaseIn**: Starts slow, accelerates toward end.
/// - **EaseOut**: Starts fast, decelerates toward end.
/// - **EaseInOut**: Slow at both ends, fast in middle.
/// - **EaseInCubic/EaseOutCubic/EaseInOutCubic**: Cubic variants (more pronounced).
///
/// # Example
///
/// ```
/// use xeno_tui::animation::Easing;
///
/// let progress = 0.5;
/// let eased = Easing::EaseOut.apply(progress);
/// assert!(eased > progress); // EaseOut is ahead of linear at midpoint
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Easing {
	/// No easing - constant speed.
	#[default]
	Linear,

	/// Quadratic ease-in: starts slow, accelerates.
	/// Formula: `t²`
	EaseIn,

	/// Quadratic ease-out: starts fast, decelerates.
	/// Formula: `1 - (1-t)²`
	EaseOut,

	/// Quadratic ease-in-out: slow at both ends.
	/// Formula: piecewise quadratic
	EaseInOut,

	/// Cubic ease-in: more pronounced acceleration.
	/// Formula: `t³`
	EaseInCubic,

	/// Cubic ease-out: more pronounced deceleration.
	/// Formula: `1 - (1-t)³`
	EaseOutCubic,

	/// Cubic ease-in-out: more pronounced at both ends.
	EaseInOutCubic,
}

impl Easing {
	/// Apply the easing function to linear progress.
	///
	/// Input `t` is clamped to `[0.0, 1.0]`.
	///
	/// # Returns
	///
	/// The eased progress value, also in `[0.0, 1.0]`.
	#[inline]
	pub fn apply(self, t: f32) -> f32 {
		let t = t.clamp(0.0, 1.0);
		match self {
			Easing::Linear => t,
			Easing::EaseIn => ease_in_quad(t),
			Easing::EaseOut => ease_out_quad(t),
			Easing::EaseInOut => ease_in_out_quad(t),
			Easing::EaseInCubic => ease_in_cubic(t),
			Easing::EaseOutCubic => ease_out_cubic(t),
			Easing::EaseInOutCubic => ease_in_out_cubic(t),
		}
	}
}

/// Quadratic ease-in: `t²`
#[inline]
pub fn ease_in_quad(t: f32) -> f32 {
	t * t
}

/// Quadratic ease-out: `1 - (1-t)²`
#[inline]
pub fn ease_out_quad(t: f32) -> f32 {
	1.0 - (1.0 - t).powi(2)
}

/// Quadratic ease-in-out.
#[inline]
pub fn ease_in_out_quad(t: f32) -> f32 {
	if t < 0.5 {
		2.0 * t * t
	} else {
		1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
	}
}

/// Cubic ease-in: `t³`
#[inline]
pub fn ease_in_cubic(t: f32) -> f32 {
	t * t * t
}

/// Cubic ease-out: `1 - (1-t)³`
#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
	1.0 - (1.0 - t).powi(3)
}

/// Cubic ease-in-out.
#[inline]
pub fn ease_in_out_cubic(t: f32) -> f32 {
	if t < 0.5 {
		4.0 * t * t * t
	} else {
		1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_linear() {
		assert_eq!(Easing::Linear.apply(0.0), 0.0);
		assert_eq!(Easing::Linear.apply(0.5), 0.5);
		assert_eq!(Easing::Linear.apply(1.0), 1.0);
	}

	#[test]
	fn test_ease_out_is_ahead() {
		// EaseOut should be ahead of linear at midpoint
		let linear = 0.5;
		let eased = Easing::EaseOut.apply(0.5);
		assert!(eased > linear);
	}

	#[test]
	fn test_ease_in_is_behind() {
		// EaseIn should be behind linear at midpoint
		let linear = 0.5;
		let eased = Easing::EaseIn.apply(0.5);
		assert!(eased < linear);
	}

	#[test]
	fn test_boundaries() {
		for easing in [
			Easing::Linear,
			Easing::EaseIn,
			Easing::EaseOut,
			Easing::EaseInOut,
			Easing::EaseInCubic,
			Easing::EaseOutCubic,
			Easing::EaseInOutCubic,
		] {
			assert_eq!(easing.apply(0.0), 0.0, "{easing:?} at t=0.0");
			assert_eq!(easing.apply(1.0), 1.0, "{easing:?} at t=1.0");
		}
	}

	#[test]
	fn test_clamps_input() {
		assert_eq!(Easing::Linear.apply(-0.5), 0.0);
		assert_eq!(Easing::Linear.apply(1.5), 1.0);
	}
}
