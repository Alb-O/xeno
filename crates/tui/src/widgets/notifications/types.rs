//! Core types for the notification system.

use core::time::Duration;

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
	/// Debug message (typically hidden in production).
	Debug,
	/// Trace message (verbose debugging).
	Trace,
}

/// Screen anchor position for notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Anchor {
	/// Top-left corner of the screen.
	TopLeft,
	/// Top-center of the screen.
	TopCenter,
	/// Top-right corner of the screen.
	TopRight,
	/// Middle-left edge of the screen.
	MiddleLeft,
	/// Center of the screen.
	MiddleCenter,
	/// Middle-right edge of the screen.
	MiddleRight,
	/// Bottom-left corner of the screen.
	BottomLeft,
	/// Bottom-center of the screen.
	BottomCenter,
	/// Bottom-right corner of the screen (default).
	#[default]
	BottomRight,
}

/// Animation style for notification entry/exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Animation {
	/// Slide in from a direction based on anchor position.
	Slide,
	/// Expand from center point, collapse on exit.
	ExpandCollapse,
	/// Fade in/out (default).
	#[default]
	Fade,
}

/// Direction for slide animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SlideDirection {
	/// Automatically determine direction based on anchor position.
	#[default]
	Auto,
	/// Slide from the top edge.
	FromTop,
	/// Slide from the bottom edge.
	FromBottom,
	/// Slide from the left edge.
	FromLeft,
	/// Slide from the right edge.
	FromRight,
	/// Slide from the top-left corner.
	FromTopLeft,
	/// Slide from the top-right corner.
	FromTopRight,
	/// Slide from the bottom-left corner.
	FromBottomLeft,
	/// Slide from the bottom-right corner.
	FromBottomRight,
}

/// Animation phase in the notification lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationPhase {
	/// Queued but not yet visible.
	#[default]
	Pending,
	/// Animating into view.
	Entering,
	/// Fully visible and waiting.
	Dwelling,
	/// Animating out of view.
	Exiting,
	/// Animation complete, ready for removal.
	Finished,
}

/// Controls automatic dismissal behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoDismiss {
	/// Never auto-dismiss; must be manually removed.
	Never,
	/// Auto-dismiss after the specified duration.
	After(Duration),
}

impl Default for AutoDismiss {
	fn default() -> Self {
		Self::After(Duration::from_secs(4))
	}
}

/// Duration specification for animation phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Timing {
	/// Use default timing for the animation type.
	#[default]
	Auto,
	/// Use a specific fixed duration.
	Fixed(Duration),
}

/// Size constraint for notifications.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeConstraint {
	/// Fixed size in terminal cells.
	Cells(u16),
	/// Percentage of available space (0.0 to 1.0).
	Percent(f32),
}

/// Behavior when notification limit is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Overflow {
	/// Remove the oldest notification to make room (default).
	#[default]
	DropOldest,
	/// Reject new notifications when at capacity.
	DropNewest,
}
