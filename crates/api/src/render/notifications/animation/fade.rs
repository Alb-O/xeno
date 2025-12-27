use tome_tui::animation::{Animatable, ease_in_quad, ease_out_quad};
use tome_tui::prelude::*;
use tome_manifest::notifications::AnimationPhase;

/// Target color when fully faded out (black).
const FADED_OUT_COLOR: Option<Color> = Some(Color::Black);
/// Base color assumed for content text (white).
const BASE_CONTENT_COLOR: Option<Color> = Some(Color::White);

/// Calculates the rectangle for fade animation (no size change).
///
/// Fade animations don't modify the rectangle size, only the colors.
///
/// # Returns
/// The unmodified `full_rect`.
pub fn calculate_rect(
	full_rect: Rect,
	_frame_area: Rect,
	_phase: AnimationPhase,
	_progress: f32,
) -> Rect {
	full_rect
}

/// Interpolates between two colors with easing for smooth fade effects.
///
/// Uses quadratic easing for natural-looking fade transitions. The easing function
/// is chosen based on whether the animation is fading in or out.
///
/// # Parameters
/// - `from`: Starting color
/// - `to`: Ending color
/// - `progress`: Animation progress from 0.0 to 1.0
/// - `is_fading_in`: Whether this is a fade-in (true) or fade-out (false) animation
///
/// # Returns
/// The interpolated color, or a fallback to `from`/`to` if interpolation isn't possible.
pub fn interpolate_color(
	from: Option<Color>,
	to: Option<Color>,
	progress: f32,
	is_fading_in: bool,
) -> Option<Color> {
	let linear_progress = progress.clamp(0.0, 1.0);
	let eased_progress = if is_fading_in {
		ease_out_quad(linear_progress)
	} else {
		ease_in_quad(linear_progress)
	};

	match (from, to) {
		(Some(from_color), Some(to_color)) => Some(from_color.lerp(&to_color, eased_progress)),
		(Some(_), None) | (None, Some(_)) => {
			if linear_progress < 0.5 { from } else { to }
		}
		(None, None) => None,
	}
}

/// Handler for fade animation color interpolation.
///
/// Provides methods for interpolating frame and content foreground colors
/// during fade transitions.
#[derive(Debug, Clone, Copy)]
pub struct FadeHandler;

impl FadeHandler {
	/// Interpolates the frame foreground color based on animation phase and progress.
	///
	/// The frame typically fades from black to the base color when fading in,
	/// and from the base color to black when fading out.
	///
	/// # Parameters
	/// - `base_fg`: The base foreground color when fully visible
	/// - `phase`: The current animation phase
	/// - `progress`: Animation progress from 0.0 to 1.0
	///
	/// # Returns
	/// The interpolated foreground color for the current state.
	pub fn interpolate_frame_foreground(
		&self,
		base_fg: Option<Color>,
		phase: AnimationPhase,
		progress: f32,
	) -> Option<Color> {
		let is_fading_in = matches!(
			phase,
			AnimationPhase::FadingIn | AnimationPhase::SlidingIn | AnimationPhase::Expanding
		);
		let (start_fg, end_fg) = match phase {
			AnimationPhase::FadingIn | AnimationPhase::SlidingIn | AnimationPhase::Expanding => {
				(FADED_OUT_COLOR, base_fg)
			}
			AnimationPhase::FadingOut | AnimationPhase::SlidingOut | AnimationPhase::Collapsing => {
				(base_fg, FADED_OUT_COLOR)
			}
			_ => return base_fg,
		};
		interpolate_color(start_fg, end_fg, progress, is_fading_in)
	}

	/// Interpolates the content foreground color based on animation phase and progress.
	///
	/// Content text fades from black to white when fading in, and from white to black
	/// when fading out.
	///
	/// # Parameters
	/// - `_base_fg`: The base foreground color (currently unused)
	/// - `phase`: The current animation phase
	/// - `progress`: Animation progress from 0.0 to 1.0
	///
	/// # Returns
	/// The interpolated content color for the current state.
	pub fn interpolate_content_foreground(
		&self,
		_base_fg: Option<Color>,
		phase: AnimationPhase,
		progress: f32,
	) -> Option<Color> {
		let is_fading_in = matches!(
			phase,
			AnimationPhase::FadingIn | AnimationPhase::SlidingIn | AnimationPhase::Expanding
		);
		let (start_fg, end_fg) = match phase {
			AnimationPhase::FadingIn | AnimationPhase::SlidingIn | AnimationPhase::Expanding => {
				(FADED_OUT_COLOR, BASE_CONTENT_COLOR)
			}
			AnimationPhase::FadingOut | AnimationPhase::SlidingOut | AnimationPhase::Collapsing => {
				(BASE_CONTENT_COLOR, FADED_OUT_COLOR)
			}
			_ => return BASE_CONTENT_COLOR,
		};
		interpolate_color(start_fg, end_fg, progress, is_fading_in)
	}
}
