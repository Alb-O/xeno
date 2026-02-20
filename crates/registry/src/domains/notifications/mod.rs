//! Notification registry.
//!
//! Type-safe notification system with compile-time checked notification keys.
//! Keys are organized by domain (editor, commands, actions, core).

use std::time::Duration;

pub use crate::core::{RegistryMetadata, RegistrySource};

#[macro_use]
#[path = "exec/macros.rs"]
mod macros;

#[path = "compile/builtins.rs"]
pub mod builtins;
#[path = "contract/def.rs"]
pub mod def;
mod domain;
#[path = "contract/entry.rs"]
pub mod entry;
#[path = "runtime/keys/mod.rs"]
pub mod keys;
#[path = "compile/link.rs"]
pub mod link;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "contract/spec.rs"]
pub mod spec;

pub use builtins::register_builtins;
pub use def::{LinkedNotificationDef, NotificationDef, NotificationInput};
pub use domain::Notifications;
pub use entry::NotificationEntry;

pub use crate::core::NotificationId;

/// Registers compiled notifications from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_notifications_spec();
	let linked = link::link_notifications(&spec);

	for def in linked {
		db.push_domain::<Notifications>(NotificationInput::Linked(def));
	}
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

/// Runtime notification instance ready to display.
#[derive(Debug, Clone)]
pub struct Notification {
	/// Canonical identifier of the notification type.
	pub id: std::sync::Arc<str>,
	/// Severity level (resolved from registry if None).
	pub level: Option<Level>,
	/// Auto-dismiss behavior (resolved from registry if None).
	pub auto_dismiss: Option<AutoDismiss>,
	/// The formatted message content.
	pub message: String,
}

impl Notification {
	/// Creates a new fully-specified notification instance.
	pub fn new(id: impl Into<std::sync::Arc<str>>, level: Level, auto_dismiss: AutoDismiss, message: impl Into<String>) -> Self {
		Self {
			id: id.into(),
			level: Some(level),
			auto_dismiss: Some(auto_dismiss),
			message: message.into(),
		}
	}

	/// Creates a pending notification that will be resolved at the sink.
	pub fn new_pending(id: impl Into<std::sync::Arc<str>>, message: impl Into<String>) -> Self {
		Self {
			id: id.into(),
			level: None,
			auto_dismiss: None,
			message: message.into(),
		}
	}

	/// Returns the notification level, or Info if not yet resolved.
	pub fn level(&self) -> Level {
		if self.level.is_none() {
			tracing::error!(id = %self.id, "Notification accessed before resolution");
		}
		self.level.unwrap_or(Level::Info)
	}

	/// Returns the auto-dismiss behavior, or default if not yet resolved.
	pub fn auto_dismiss(&self) -> AutoDismiss {
		if self.auto_dismiss.is_none() {
			tracing::error!(id = %self.id, "Notification accessed before resolution");
		}
		self.auto_dismiss.unwrap_or(AutoDismiss::DEFAULT)
	}

	/// Resolves this notification against the provided registry.
	/// Returns true if resolved successfully.
	pub fn resolve(&mut self, db: &crate::db::RegistryCatalog) -> bool {
		if let Some(entry) = db.notifications_reg().get(&self.id) {
			self.level = Some(entry.level);
			self.auto_dismiss = Some(entry.auto_dismiss);
			true
		} else {
			tracing::error!(id = %self.id, "Failed to resolve notification ID");
			false
		}
	}
}

/// Typed key referencing a notification definition with a static message.
#[derive(Clone, Copy)]
pub struct NotificationKey {
	id: &'static str,
	message: &'static str,
}

impl NotificationKey {
	/// Creates a new notification key with a static message.
	pub const fn new(id: &'static str, message: &'static str) -> Self {
		Self { id, message }
	}

	/// Creates a notification instance from this key.
	pub fn emit(self) -> Notification {
		Notification::new_pending(self.id, self.message)
	}
}

impl core::fmt::Debug for NotificationKey {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("NotificationKey").field("id", &self.id).field("message", &self.message).finish()
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

#[cfg(feature = "minimal")]
pub use crate::db::NOTIFICATIONS;

/// Returns all registered notification definitions.
#[cfg(feature = "minimal")]
pub fn all() -> Vec<crate::core::RegistryRef<NotificationEntry, NotificationId>> {
	NOTIFICATIONS.snapshot_guard().iter_refs().collect()
}
