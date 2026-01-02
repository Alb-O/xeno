//! Extension traits for notification methods.
//!
//! These traits provide convenience methods for displaying notifications
//! at different severity levels.

use crate::editor_ctx::MessageAccess;

/// Extension trait for displaying informational notifications.
pub trait NotifyINFOExt: MessageAccess {
	/// Displays an informational notification.
	fn info(&mut self, msg: &str) {
		self.notify("info", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifyINFOExt for T {}

/// Extension trait for displaying warning notifications.
pub trait NotifyWARNExt: MessageAccess {
	/// Displays a warning notification.
	fn warn(&mut self, msg: &str) {
		self.notify("warn", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifyWARNExt for T {}

/// Extension trait for displaying error notifications.
pub trait NotifyERRORExt: MessageAccess {
	/// Displays an error notification.
	fn error(&mut self, msg: &str) {
		self.notify("error", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifyERRORExt for T {}

/// Extension trait for displaying success notifications.
pub trait NotifySUCCESSExt: MessageAccess {
	/// Displays a success notification.
	fn success(&mut self, msg: &str) {
		self.notify("success", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifySUCCESSExt for T {}

/// Extension trait for displaying debug notifications.
pub trait NotifyDEBUGExt: MessageAccess {
	/// Displays a debug notification.
	fn debug(&mut self, msg: &str) {
		self.notify("debug", msg);
	}
}

impl<T: MessageAccess + ?Sized> NotifyDEBUGExt for T {}
