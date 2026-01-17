//! Notification registry.
//!
//! Type-safe notification system with compile-time checked notification keys.
//! Keys are organized by domain (editor, commands, actions, core).
//!
//! # Usage
//!
//! ```ignore
//! use xeno_registry_notifications::keys;
//!
//! // Static message notifications
//! ctx.emit(keys::buffer_readonly);
//!
//! // Parameterized notifications
//! ctx.emit(keys::yanked_chars::call(42));
//! ctx.emit(keys::file_saved::call(&path));
//! ```

use std::sync::LazyLock;
use std::time::Duration;

pub use xeno_registry_core::{Key, RegistryMetadata, RegistrySource};

mod actions;
mod builtins;
mod commands;
mod editor;
mod runtime;

/// All notification keys, organized by domain.
pub mod keys {
	pub use crate::actions::keys::*;
	pub use crate::builtins::keys::*;
	pub use crate::commands::keys::*;
	pub use crate::editor::keys::*;
	pub use crate::runtime::keys::*;
}

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
///
/// This contains the metadata for a notification type, but not the message
/// content itself. Messages are provided at emit time via [`Notification`].
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

/// Registry of all notification definitions.
pub static NOTIFICATIONS: LazyLock<Vec<&NotificationDef>> = LazyLock::new(|| {
	let mut defs = Vec::new();
	defs.extend_from_slice(actions::NOTIFICATIONS);
	defs.extend_from_slice(builtins::NOTIFICATIONS);
	defs.extend_from_slice(commands::NOTIFICATIONS);
	defs.extend_from_slice(editor::NOTIFICATIONS);
	defs.extend_from_slice(runtime::NOTIFICATIONS);
	defs
});

/// Runtime notification instance ready to display.
///
/// Created by emitting a [`NotificationKey`] or calling a parameterized
/// notification builder.
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
///
/// Use `.emit()` to create a [`Notification`] instance, or pass directly
/// to `ctx.emit()` which will call `into_notification()` automatically.
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
///
/// Implemented by:
/// - [`Notification`] (identity)
/// - [`NotificationKey`] (static message)
///
/// This enables `ctx.emit()` to accept both pre-built notifications
/// and notification keys directly.
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
