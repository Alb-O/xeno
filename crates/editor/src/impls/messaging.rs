//! Notification display for the editor.

use xeno_registry::notifications::Notification;

use crate::impls::Editor;
use crate::notifications::NotificationCenter;

pub(super) fn push_notification(notifications: &mut NotificationCenter, notification: Notification) {
	notifications.push(notification);
}

impl Editor {
	/// Emits a typed notification.
	///
	/// Accepts anything that converts to a [`Notification`]:
	/// * `keys::BUFFER_READONLY` (static message key)
	/// * `keys::yanked_chars(42)` (parameterized builder)
	///
	/// # Examples
	///
	/// ```ignore
	/// use xeno_registry::notifications::keys;
	///
	/// editor.notify(keys::BUFFER_READONLY);
	/// editor.notify(keys::regex_error(&err));
	/// ```
	pub fn notify(&mut self, notification: impl Into<Notification>) {
		self.show_notification(notification.into());
	}

	/// Shows a typed notification (internal).
	pub fn show_notification(&mut self, notification: Notification) {
		push_notification(&mut self.state.notifications, notification);
	}

	/// Clears all visible notifications.
	pub fn clear_all_notifications(&mut self) {
		self.state.notifications.clear();
	}
}
