//! Notification registry.
//!
//! Type-safe notification system with compile-time checked notification keys.
//! Keys are organized by domain (editor, commands, actions, core).

use std::time::Duration;

pub use crate::core::{RegistryMetadata, RegistrySource};

#[macro_use]
mod macros;

pub mod builtins;
pub mod keys;

pub use builtins::register_builtins;

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

// Re-export macros
pub use crate::{notif, notif_alias};

/// Severity level for notifications.
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
	/// Success message.
	Success,
}

/// Controls automatic dismissal of notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoDismiss {
	/// Notification remains visible until manually dismissed.
	Never,
	/// Notification automatically dismisses after the specified duration.
	After(Duration),
}

impl AutoDismiss {
	/// Default auto-dismiss duration (4 seconds).
	pub const DEFAULT: Self = Self::After(Duration::from_secs(4));
}

impl Default for AutoDismiss {
	fn default() -> Self {
		Self::DEFAULT
	}
}

/// Static notification definition registered in the notification list.
#[derive(Debug)]
pub struct NotificationDef {
	/// Unique identifier for this notification type.
	pub id: &'static str,
	/// Severity level.
	pub level: Level,
	/// Auto-dismiss behavior.
	pub auto_dismiss: AutoDismiss,
	/// Where this notification was defined.
	pub source: RegistrySource,
}

impl NotificationDef {
	/// Creates a new notification definition.
	pub const fn new(
		id: &'static str,
		level: Level,
		auto_dismiss: AutoDismiss,
		source: RegistrySource,
	) -> Self {
		Self {
			id,
			level,
			auto_dismiss,
			source,
		}
	}
}

/// Runtime notification instance ready to display.
#[derive(Debug, Clone)]
pub struct Notification {
	/// Reference to the static definition.
	pub def: &'static NotificationDef,
	/// The formatted message content.
	pub message: String,
}

impl Notification {
	/// Creates a new notification instance.
	pub fn new(def: &'static NotificationDef, message: impl Into<String>) -> Self {
		Self {
			def,
			message: message.into(),
		}
	}

	/// Returns the notification level.
	pub fn level(&self) -> Level {
		self.def.level
	}

	/// Returns the auto-dismiss behavior.
	pub fn auto_dismiss(&self) -> AutoDismiss {
		self.def.auto_dismiss
	}
}

/// Typed key referencing a notification definition with a static message.
#[derive(Clone, Copy)]
pub struct NotificationKey {
	def: &'static NotificationDef,
	message: &'static str,
}

impl NotificationKey {
	/// Creates a new notification key with a static message.
	pub const fn new(def: &'static NotificationDef, message: &'static str) -> Self {
		Self { def, message }
	}

	/// Creates a notification instance from this key.
	pub fn emit(self) -> Notification {
		Notification::new(self.def, self.message)
	}

	/// Returns the notification level.
	pub fn level(self) -> Level {
		self.def.level
	}
}

impl core::fmt::Debug for NotificationKey {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("NotificationKey")
			.field("id", &self.def.id)
			.field("message", &self.message)
			.finish()
	}
}

/// Trait for anything that can become a [`Notification`].
pub trait IntoNotification {
	/// Converts this value into a notification.
	fn into_notification(self) -> Notification;
}

impl IntoNotification for Notification {
	fn into_notification(self) -> Notification {
		self
	}
}

impl IntoNotification for NotificationKey {
	fn into_notification(self) -> Notification {
		self.emit()
	}
}

impl From<NotificationKey> for Notification {
	fn from(key: NotificationKey) -> Self {
		key.emit()
	}
}

#[cfg(feature = "db")]
pub use crate::db::NOTIFICATIONS;

/// Returns all registered notification definitions.
#[cfg(feature = "db")]
pub fn all() -> impl Iterator<Item = &'static NotificationDef> {
	NOTIFICATIONS.iter().copied()
}
