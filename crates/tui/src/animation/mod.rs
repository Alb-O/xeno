//! Animation primitives for smooth transitions.
//!
//! Provides the [`Animatable`] trait for types that can be interpolated,
//! [`Easing`] functions for animation curves, and [`Tween`] for time-based
//! animations.
//!
//! # Example
//!
//! ```
//! use std::time::Duration;
//! use evildoer_tui::animation::{Animatable, Easing, Tween};
//!
//! // Animate a float from 0.0 to 100.0 over 500ms with ease-out
//! let tween = Tween::new(0.0f32, 100.0f32, Duration::from_millis(500))
//!     .with_easing(Easing::EaseOut);
//!
//! // Get current value (depends on elapsed time)
//! let current = tween.value();
//! ```

mod easing;
mod lerp;
mod tween;

pub use easing::{
	Easing, ease_in_cubic, ease_in_out_cubic, ease_in_out_quad, ease_in_quad, ease_out_cubic,
	ease_out_quad,
};
pub use lerp::Animatable;
pub use tween::{ToggleTween, Tween};
