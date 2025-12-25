use std::time::Duration;

use ratatui::layout::Rect;
use thiserror::Error;

/// Screen position from which notifications expand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Anchor {
	TopLeft,
	TopCenter,
	TopRight,
	MiddleLeft,
	MiddleCenter,
	MiddleRight,
	BottomLeft,
	BottomCenter,
	/// Default anchor position. Notifications expand from bottom-right.
	#[default]
	BottomRight,
}

/// Animation style for notification entry and exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Animation {
	/// Slide animation from a direction (default).
	#[default]
	Slide,
	/// Expand/collapse animation.
	ExpandCollapse,
	/// Fade animation.
	Fade,
}

/// Animation phase tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationPhase {
	#[default]
	Pending,
	SlidingIn,
	Expanding,
	FadingIn,
	Dwelling,
	SlidingOut,
	Collapsing,
	FadingOut,
	Finished,
}

/// Controls automatic dismissal of notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoDismiss {
	/// Notification remains visible until manually dismissed.
	Never,
	/// Notification automatically dismisses after the specified duration.
	After(Duration),
}

impl Default for AutoDismiss {
	fn default() -> Self {
		Self::After(Duration::from_secs(4))
	}
}

/// Severity level of a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Level {
	/// Informational message (default).
	#[default]
	Info,
	/// Warning message.
	Warn,
	/// Error message.
	Error,
	/// Debug message.
	Debug,
	/// Trace message.
	Trace,
}

/// Behavior when notification limit is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Overflow {
	/// Discard the oldest notification when limit is reached (default).
	#[default]
	DiscardOldest,
	/// Discard the newest notification when limit is reached.
	DiscardNewest,
}

/// Constraint on notification dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeConstraint {
	/// Absolute size in terminal cells/characters.
	Absolute(u16),
	/// Percentage of available screen space (0.0 to 1.0).
	Percentage(f32),
}

/// Direction from which a notification slides in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SlideDirection {
	/// Auto-select direction based on anchor point (default).
	#[default]
	Default,
	FromTop,
	FromBottom,
	FromLeft,
	FromRight,
	FromTopLeft,
	FromTopRight,
	FromBottomLeft,
	FromBottomRight,
}

/// Animation duration specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Timing {
	/// Fixed duration specified by user.
	Fixed(Duration),
	/// Automatically calculated duration.
	#[default]
	Auto,
}

/// Parameters for sliding animations.
#[derive(Debug, Clone, Copy)]
pub struct SlideParams {
	pub full_rect: Rect,
	pub frame_area: Rect,
	pub progress: f32,
	pub phase: AnimationPhase,
	pub anchor: Anchor,
	pub slide_direction: SlideDirection,
	pub custom_slide_in_start_pos: Option<(f32, f32)>,
	pub custom_slide_out_end_pos: Option<(f32, f32)>,
}

/// Errors specific to the notification system.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NotificationError {
	/// Invalid configuration provided.
	#[error("Invalid configuration: {0}")]
	InvalidConfig(String),
	/// Content exceeds size limits.
	#[error("Content too large: {0} chars exceeds limit of {1} chars")]
	ContentTooLarge(usize, usize),
}
