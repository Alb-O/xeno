use std::time::Duration;

use linkme::distributed_slice;
use thiserror::Error;

use crate::RegistrySource;

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

pub struct NotificationTypeDef {
	pub id: &'static str,
	pub name: &'static str,
	pub level: Level,
	pub icon: Option<&'static str>,
	pub semantic: &'static str,
	pub auto_dismiss: AutoDismiss,
	pub animation: Animation,
	pub timing: (Timing, Timing, Timing), // In, Dwell, Out
	pub priority: i16,
	pub source: RegistrySource,
}

#[distributed_slice]
pub static NOTIFICATION_TYPES: [NotificationTypeDef];

pub fn find_notification_type(name: &str) -> Option<&'static NotificationTypeDef> {
	NOTIFICATION_TYPES.iter().find(|t| t.name == name)
}
