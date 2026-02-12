use xeno_editor::Editor;
use xeno_registry::notifications::{AutoDismiss, Level, Notification};
use xeno_registry::themes::ThemeColors;
use xeno_tui::style::Style;
use xeno_tui::widgets::icon::presets as icon_presets;
use xeno_tui::widgets::notifications::{self as notif, Anchor, Toast, ToastIcon};

fn map_notification_to_toast(colors: ThemeColors, notification: Notification) -> Toast {
	let level = notification.level();
	let auto_dismiss = notification.auto_dismiss();
	let (semantic, icon_glyph) = match level {
		Level::Info => ("info", icon_presets::INFO),
		Level::Warn => ("warning", icon_presets::WARNING),
		Level::Error => ("error", icon_presets::ERROR),
		Level::Success => ("success", icon_presets::SUCCESS),
		Level::Debug => ("dim", icon_presets::DEBUG),
	};
	let notif_style: Style = colors.notification_style(semantic);
	let accent = notif_style.fg.unwrap_or_default();
	Toast::new(notification.message)
		.anchor(Anchor::TopRight)
		.style(notif_style)
		.border_style(Style::default().fg(accent))
		.icon(ToastIcon::new(icon_glyph).style(Style::default().fg(accent)))
		.animation(notif::Animation::Fade)
		.auto_dismiss(match auto_dismiss {
			AutoDismiss::Never => notif::AutoDismiss::Never,
			AutoDismiss::After(d) => notif::AutoDismiss::After(d),
		})
}

pub fn render(ed: &mut Editor, doc_area: xeno_tui::layout::Rect, buffer: &mut xeno_tui::buffer::Buffer) {
	let theme_colors = ed.config().theme.colors;
	let notifications = ed.notifications_mut();
	for notification in notifications.take_pending() {
		let toast = map_notification_to_toast(theme_colors, notification);
		notifications.toast_manager_mut().push(toast);
	}

	let mut notifications_area = doc_area;
	notifications_area.height = notifications_area.height.saturating_sub(1);
	notifications_area.width = notifications_area.width.saturating_sub(1);
	notifications.toast_manager_mut().render(notifications_area, buffer);
}
