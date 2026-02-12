//! Editor notification center wrapper.
//!
//! Keeps toast styling/rendering backend-specific details isolated from editor
//! state ownership and call sites.

use std::time::Duration;

use xeno_registry::notifications::{AutoDismiss, Level, Notification};
use xeno_tui::style::Style;
use xeno_tui::widgets::icon::presets as icon_presets;
use xeno_tui::widgets::notifications::{self as notif, Anchor, Overflow, Toast, ToastIcon, ToastManager};

use crate::geometry::Rect;
use crate::types::Config;

pub struct NotificationCenter {
	inner: ToastManager,
}

impl Default for NotificationCenter {
	fn default() -> Self {
		Self::new()
	}
}

impl NotificationCenter {
	pub fn new() -> Self {
		Self {
			inner: ToastManager::new().max_visible(Some(5)).overflow(Overflow::DropOldest),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	pub fn tick(&mut self, delta: Duration) {
		self.inner.tick(delta);
	}

	pub fn clear(&mut self) {
		self.inner.clear();
	}

	pub fn push(&mut self, config: &Config, notification: Notification) {
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

		self.inner.push(toast);
	}

	pub fn render(&mut self, area: Rect, buffer: &mut xeno_tui::buffer::Buffer) {
		self.inner.render(area.into(), buffer);
	}
}
