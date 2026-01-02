//! Notification registry
//!
//! Defines notification types and compile-time registrations.

use std::time::Duration;

use linkme::distributed_slice;
use thiserror::Error;

mod impls;

pub use evildoer_registry_motions::{RegistryMetadata, RegistrySource, impl_registry_metadata};

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

/// Screen position from which notifications expand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
pub enum Animation {
	/// Slide animation from a direction.
	Slide,
	/// Expand/collapse animation.
	ExpandCollapse,
	/// Fade animation (default).
	#[default]
	Fade,
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

/// Animation duration specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Timing {
	/// Fixed duration specified by user.
	Fixed(Duration),
	/// Automatically calculated duration.
	#[default]
	Auto,
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

/// Animation phase tracking for notification lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationPhase {
	/// Notification is queued but not yet visible.
	#[default]
	Pending,
	/// Sliding into view.
	SlidingIn,
	/// Expanding from anchor point.
	Expanding,
	/// Fading into visibility.
	FadingIn,
	/// Fully visible and waiting.
	Dwelling,
	/// Sliding out of view.
	SlidingOut,
	/// Collapsing back to anchor point.
	Collapsing,
	/// Fading out of visibility.
	FadingOut,
	/// Animation complete, ready for removal.
	Finished,
}

/// Behavior when notification limit is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Overflow {
	/// Remove the oldest notification to make room.
	#[default]
	DiscardOldest,
	/// Reject new notifications when at capacity.
	DiscardNewest,
}

/// Constraint on notification dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeConstraint {
	/// Fixed size in terminal cells.
	Absolute(u16),
	/// Percentage of available space (0.0 to 1.0).
	Percentage(f32),
}

/// Direction from which a notification slides in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SlideDirection {
	/// Infer direction from anchor position.
	#[default]
	Default,
	/// Slide from the top edge.
	FromTop,
	/// Slide from the bottom edge.
	FromBottom,
	/// Slide from the left edge.
	FromLeft,
	/// Slide from the right edge.
	FromRight,
	/// Slide from top-left corner.
	FromTopLeft,
	/// Slide from top-right corner.
	FromTopRight,
	/// Slide from bottom-left corner.
	FromBottomLeft,
	/// Slide from bottom-right corner.
	FromBottomRight,
}

pub struct NotificationTypeDef {
	pub id: &'static str,
	pub name: &'static str,
	pub level: Level,
	pub icon: Option<&'static str>,
	pub semantic: &'static str,
	pub auto_dismiss: AutoDismiss,
	pub animation: Animation,
	/// Animation timing phases: (In, Dwell, Out)
	pub timing: (Timing, Timing, Timing),
	pub priority: i16,
	pub source: RegistrySource,
}

#[distributed_slice]
pub static NOTIFICATION_TYPES: [NotificationTypeDef];

pub fn find_notification_type(name: &str) -> Option<&'static NotificationTypeDef> {
	NOTIFICATION_TYPES.iter().find(|t| t.name == name)
}

impl_registry_metadata!(NotificationTypeDef);
