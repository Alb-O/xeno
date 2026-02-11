//! Notification display for the editor.

use xeno_registry::notifications::{AutoDismiss, Level, Notification};
use xeno_tui::style::Style;
use xeno_tui::widgets::icon::presets as icon_presets;
use xeno_tui::widgets::notifications::{self as notif, Anchor, Toast, ToastIcon, ToastManager};

use crate::impls::Editor;
use crate::types::Config;

pub(super) fn push_notification(config: &Config, notifications: &mut ToastManager, notification: Notification) {
	let level = notification.level();
	let auto_dismiss = notification.auto_dismiss();

	let (semantic, icon_glyph) = match level {
		Level::Info => ("info", icon_presets::INFO),
		Level::Warn => ("warning", icon_presets::WARNING),
		Level::Error => ("error", icon_presets::ERROR),
		Level::Success => ("success", icon_presets::SUCCESS),
		Level::Debug => ("dim", icon_presets::DEBUG),
	};

	let notif_style: Style = config.theme.colors.notification_style(semantic);
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

	notifications.push(toast);
}

impl Editor {
	/// Emits a typed notification.
	///
	/// Accepts anything that converts to a [`Notification`]:
	/// - `keys::BUFFER_READONLY` (static message key)
	/// - `keys::yanked_chars(42)` (parameterized builder)
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
		push_notification(&self.state.config, &mut self.state.notifications, notification);
	}

	/// Clears all visible notifications.
	pub fn clear_all_notifications(&mut self) {
		self.state.notifications.clear();
	}
}
