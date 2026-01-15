//! Notification display for the editor.

use xeno_registry_notifications::{AutoDismiss, Level, Notification};
use xeno_tui::style::Style;
use xeno_tui::widgets::icon::presets as icon_presets;
use xeno_tui::widgets::notifications::{self as notif, Anchor, Toast, ToastIcon};

use crate::editor::Editor;

impl Editor {
	/// Emits a typed notification.
	///
	/// Accepts anything that converts to a [`Notification`]:
	/// - `keys::buffer_readonly` (static message key)
	/// - `keys::yanked_chars::call(42)` (parameterized builder)
	///
	/// # Examples
	///
	/// ```ignore
	/// use xeno_registry_notifications::keys;
	///
	/// editor.notify(keys::buffer_readonly);
	/// editor.notify(keys::regex_error::call(&err));
	/// ```
	pub fn notify(&mut self, notification: impl Into<Notification>) {
		self.show_notification(notification.into());
	}

	/// Shows a typed notification (internal).
	pub fn show_notification(&mut self, notification: Notification) {
		let level = notification.level();
		let auto_dismiss = notification.auto_dismiss();

		// Get style based on level
		let (semantic, icon_glyph) = match level {
			Level::Info => ("info", icon_presets::INFO),
			Level::Warn => ("warning", icon_presets::WARNING),
			Level::Error => ("error", icon_presets::ERROR),
			Level::Success => ("success", icon_presets::SUCCESS),
			Level::Debug => ("dim", icon_presets::DEBUG),
		};

		let notif_style: Style = self.config.theme.colors.notification_style(semantic);
		let accent = notif_style.fg.unwrap_or_default();

		let toast = Toast::new(notification.message)
			.anchor(Anchor::TopRight)
			.style(notif_style)
			.border_style(Style::default().fg(accent))
			.icon(ToastIcon::new(icon_glyph).style(Style::default().fg(accent)))
			.animation(notif::Animation::Fade)
			.auto_dismiss(match auto_dismiss {
				AutoDismiss::Never => notif::AutoDismiss::Never,
				AutoDismiss::After(d) => notif::AutoDismiss::After(d),
			});

		self.notifications.push(toast);
	}

	/// Clears all visible notifications.
	pub fn clear_all_notifications(&mut self) {
		self.notifications.clear();
	}
}
