//! Notification registry
//!
//! Defines notification types and compile-time registrations.

use std::time::Duration;

use linkme::distributed_slice;
use thiserror::Error;

mod impls;

pub use evildoer_registry_motions::{impl_registry_metadata, RegistryMetadata, RegistrySource};

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

/// Definition of a notification type with default styling and behavior.
pub struct NotificationTypeDef {
	/// Unique identifier.
	pub id: &'static str,
	/// Display name.
	pub name: &'static str,
	/// Severity level.
	pub level: Level,
	/// Optional icon glyph.
	pub icon: Option<&'static str>,
	/// Semantic category (e.g., "save", "error", "lsp").
	pub semantic: &'static str,
	/// Auto-dismiss behavior.
	pub auto_dismiss: AutoDismiss,
	/// Animation style.
	pub animation: Animation,
	/// Animation timing phases: (In, Dwell, Out)
	pub timing: (Timing, Timing, Timing),
	/// Registration priority (lower = earlier).
	pub priority: i16,
	/// Origin of the registration.
	pub source: RegistrySource,
}

/// Registry of all notification type definitions.
#[distributed_slice]
pub static NOTIFICATION_TYPES: [NotificationTypeDef];

/// Finds a notification type by name.
pub fn find_notification_type(name: &str) -> Option<&'static NotificationTypeDef> {
	NOTIFICATION_TYPES.iter().find(|t| t.name == name)
}

impl_registry_metadata!(NotificationTypeDef);
