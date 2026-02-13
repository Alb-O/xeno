use std::time::Duration;

use xeno_editor::{Editor, NotificationRenderAutoDismiss, NotificationRenderItem, NotificationRenderLevel, ThemeColors};
use xeno_tui::style::Style;
use xeno_tui::widgets::icon::presets as icon_presets;
use xeno_tui::widgets::notifications::{self as notif, Anchor, Overflow, Toast, ToastIcon, ToastManager};

pub struct FrontendNotifications {
	clear_epoch: u64,
	toasts: ToastManager,
}

impl Default for FrontendNotifications {
	fn default() -> Self {
		Self::new()
	}
}

impl FrontendNotifications {
	pub fn new() -> Self {
		Self {
			clear_epoch: 0,
			toasts: ToastManager::new().max_visible(Some(5)).overflow(Overflow::DropOldest),
		}
	}

	pub fn tick(&mut self, delta: Duration) {
		self.toasts.tick(delta);
	}

	pub fn has_active_toasts(&self) -> bool {
		!self.toasts.is_empty()
	}
}

fn map_notification_to_toast(colors: ThemeColors, notification: NotificationRenderItem) -> Toast {
	let level = notification.level;
	let auto_dismiss = notification.auto_dismiss;
	let (semantic, icon_glyph) = match level {
		NotificationRenderLevel::Info => ("info", icon_presets::INFO),
		NotificationRenderLevel::Warn => ("warning", icon_presets::WARNING),
		NotificationRenderLevel::Error => ("error", icon_presets::ERROR),
		NotificationRenderLevel::Success => ("success", icon_presets::SUCCESS),
		NotificationRenderLevel::Debug => ("dim", icon_presets::DEBUG),
	};
	let notif_style: Style = colors.notification_style(semantic).into();
	let accent = notif_style.fg.unwrap_or_default();
	Toast::new(notification.message)
		.anchor(Anchor::TopRight)
		.style(notif_style)
		.border_style(Style::default().fg(accent))
		.icon(ToastIcon::new(icon_glyph).style(Style::default().fg(accent)))
		.animation(notif::Animation::Fade)
		.auto_dismiss(match auto_dismiss {
			NotificationRenderAutoDismiss::Never => notif::AutoDismiss::Never,
			NotificationRenderAutoDismiss::After(d) => notif::AutoDismiss::After(d),
		})
}

pub fn render(ed: &mut Editor, state: &mut FrontendNotifications, doc_area: xeno_tui::layout::Rect, buffer: &mut xeno_tui::buffer::Buffer) {
	let theme_colors = ed.config().theme.colors;
	let clear_epoch = ed.notifications_clear_epoch();
	if clear_epoch != state.clear_epoch {
		state.toasts.clear();
		state.clear_epoch = clear_epoch;
	}

	for notification in ed.take_notification_render_items() {
		let toast = map_notification_to_toast(theme_colors, notification);
		state.toasts.push(toast);
	}

	let mut notifications_area = doc_area;
	notifications_area.height = notifications_area.height.saturating_sub(1);
	notifications_area.width = notifications_area.width.saturating_sub(1);
	state.toasts.render(notifications_area, buffer);
}
