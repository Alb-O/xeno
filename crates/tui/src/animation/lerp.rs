//! The [`Animatable`] trait for types that support interpolation.

/// A type that can be linearly interpolated.
///
/// Implementors should provide smooth transitions between values, enabling
/// animation systems to tween between states.
///
/// # Example
///
/// ```
/// use xeno_tui::animation::Animatable;
///
/// let start = 0.0f32;
/// let end = 100.0f32;
///
/// assert_eq!(start.lerp(&end, 0.0), 0.0);
/// assert_eq!(start.lerp(&end, 0.5), 50.0);
/// assert_eq!(start.lerp(&end, 1.0), 100.0);
/// ```
pub trait Animatable: Clone {
	/// Linearly interpolate between `self` and `target`.
	///
	/// The parameter `t` represents progress:
	/// * `t = 0.0` returns `self`
	/// * `t = 1.0` returns `target`
	/// * Values in between return a proportional blend
	///
	/// Implementations should clamp `t` to `[0.0, 1.0]`.
	fn lerp(&self, target: &Self, t: f32) -> Self;
}

impl Animatable for f32 {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0);
		self + (target - self) * t
	}
}

impl Animatable for f64 {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0) as f64;
		self + (target - self) * t
	}
}

impl Animatable for u8 {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0);
		let result = *self as f32 + (*target as f32 - *self as f32) * t;
		result.round() as u8
	}
}

impl Animatable for u16 {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0);
		let result = *self as f32 + (*target as f32 - *self as f32) * t;
		result.round() as u16
	}
}

impl Animatable for i16 {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0);
		let result = *self as f32 + (*target as f32 - *self as f32) * t;
		result.round() as i16
	}
}

impl Animatable for i32 {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0);
		let result = *self as f64 + (*target as f64 - *self as f64) * t as f64;
		result.round() as i32
	}
}

/// RGB color tuple (r, g, b).
impl Animatable for (u8, u8, u8) {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		(self.0.lerp(&target.0, t), self.1.lerp(&target.1, t), self.2.lerp(&target.2, t))
	}
}

/// RGBA color tuple (r, g, b, a).
impl Animatable for (u8, u8, u8, u8) {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		(
			self.0.lerp(&target.0, t),
			self.1.lerp(&target.1, t),
			self.2.lerp(&target.2, t),
			self.3.lerp(&target.3, t),
		)
	}
}

/// 2D point/vector.
impl Animatable for (f32, f32) {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		(self.0.lerp(&target.0, t), self.1.lerp(&target.1, t))
	}
}

/// Rectangle as (x, y, width, height).
impl Animatable for (u16, u16, u16, u16) {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		(
			self.0.lerp(&target.0, t),
			self.1.lerp(&target.1, t),
			self.2.lerp(&target.2, t),
			self.3.lerp(&target.3, t),
		)
	}
}

impl Animatable for crate::layout::Rect {
	#[inline]
	fn lerp(&self, target: &Self, t: f32) -> Self {
		Self {
			x: self.x.lerp(&target.x, t),
			y: self.y.lerp(&target.y, t),
			width: self.width.lerp(&target.width, t),
			height: self.height.lerp(&target.height, t),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_f32_lerp() {
		assert_eq!(0.0f32.lerp(&100.0, 0.0), 0.0);
		assert_eq!(0.0f32.lerp(&100.0, 0.5), 50.0);
		assert_eq!(0.0f32.lerp(&100.0, 1.0), 100.0);
	}

	#[test]
	fn test_f32_lerp_clamps() {
		assert_eq!(0.0f32.lerp(&100.0, -0.5), 0.0);
		assert_eq!(0.0f32.lerp(&100.0, 1.5), 100.0);
	}

	#[test]
	fn test_u8_lerp() {
		assert_eq!(0u8.lerp(&255, 0.0), 0);
		assert_eq!(0u8.lerp(&255, 0.5), 128);
		assert_eq!(0u8.lerp(&255, 1.0), 255);
	}

	#[test]
	fn test_rgb_lerp() {
		let black = (0u8, 0u8, 0u8);
		let white = (255u8, 255u8, 255u8);

		assert_eq!(black.lerp(&white, 0.0), (0, 0, 0));
		assert_eq!(black.lerp(&white, 0.5), (128, 128, 128));
		assert_eq!(black.lerp(&white, 1.0), (255, 255, 255));
	}

	#[test]
	fn test_point_lerp() {
		let start = (0.0f32, 0.0f32);
		let end = (100.0f32, 50.0f32);

		assert_eq!(start.lerp(&end, 0.5), (50.0, 25.0));
	}
}
