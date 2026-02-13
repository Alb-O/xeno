//! Editor notification center wrapper.
//!
//! Owns typed notification queueing for frontend presentation layers.
//!
//! Frontend crates are responsible for toast lifecycle state, visual mapping,
//! and rendering.

use std::collections::VecDeque;
use std::time::Duration;

use xeno_registry::notifications::Notification;

pub(crate) struct NotificationCenter {
	pending: VecDeque<Notification>,
	clear_epoch: u64,
}

/// Frontend-facing severity level for notification rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationRenderLevel {
	Info,
	Warn,
	Error,
	Debug,
	Success,
}

/// Frontend-facing auto-dismiss policy for notification rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationRenderAutoDismiss {
	Never,
	After(Duration),
}

/// Data-only notification item consumed by frontend renderers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationRenderItem {
	pub message: String,
	pub level: NotificationRenderLevel,
	pub auto_dismiss: NotificationRenderAutoDismiss,
}

impl From<xeno_registry::notifications::Level> for NotificationRenderLevel {
	fn from(level: xeno_registry::notifications::Level) -> Self {
		match level {
			xeno_registry::notifications::Level::Info => Self::Info,
			xeno_registry::notifications::Level::Warn => Self::Warn,
			xeno_registry::notifications::Level::Error => Self::Error,
			xeno_registry::notifications::Level::Debug => Self::Debug,
			xeno_registry::notifications::Level::Success => Self::Success,
		}
	}
}

impl From<xeno_registry::notifications::AutoDismiss> for NotificationRenderAutoDismiss {
	fn from(auto_dismiss: xeno_registry::notifications::AutoDismiss) -> Self {
		match auto_dismiss {
			xeno_registry::notifications::AutoDismiss::Never => Self::Never,
			xeno_registry::notifications::AutoDismiss::After(duration) => Self::After(duration),
		}
	}
}

impl From<Notification> for NotificationRenderItem {
	fn from(notification: Notification) -> Self {
		let level = notification.level();
		let auto_dismiss = notification.auto_dismiss();
		Self {
			message: notification.message,
			level: level.into(),
			auto_dismiss: auto_dismiss.into(),
		}
	}
}

impl Default for NotificationCenter {
	fn default() -> Self {
		Self::new()
	}
}

impl NotificationCenter {
	pub(crate) fn new() -> Self {
		Self {
			pending: VecDeque::new(),
			clear_epoch: 0,
		}
	}

	pub(crate) fn clear(&mut self) {
		self.pending.clear();
		self.clear_epoch = self.clear_epoch.wrapping_add(1);
	}

	pub(crate) fn push(&mut self, notification: Notification) {
		self.pending.push_back(notification);
	}

	pub(crate) fn take_pending(&mut self) -> Vec<Notification> {
		self.pending.drain(..).collect()
	}

	pub(crate) fn take_pending_render_items(&mut self) -> Vec<NotificationRenderItem> {
		self.take_pending().into_iter().map(NotificationRenderItem::from).collect()
	}

	pub(crate) fn clear_epoch(&self) -> u64 {
		self.clear_epoch
	}
}

#[cfg(test)]
mod tests;
