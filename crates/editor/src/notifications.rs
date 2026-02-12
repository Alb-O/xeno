//! Editor notification center wrapper.
//!
//! Owns notification queueing and toast lifecycle state.
//!
//! Frontend crates are responsible for mapping typed notifications to concrete
//! toast presentation (styles/icons/layout) and rendering.

use std::collections::VecDeque;
use std::time::Duration;

use xeno_registry::notifications::Notification;
use xeno_tui::widgets::notifications::{Overflow, ToastManager};

pub struct NotificationCenter {
	pending: VecDeque<Notification>,
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
			pending: VecDeque::new(),
			inner: ToastManager::new().max_visible(Some(5)).overflow(Overflow::DropOldest),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.pending.is_empty() && self.inner.is_empty()
	}

	pub fn tick(&mut self, delta: Duration) {
		self.inner.tick(delta);
	}

	pub fn clear(&mut self) {
		self.pending.clear();
		self.inner.clear();
	}

	pub fn push(&mut self, notification: Notification) {
		self.pending.push_back(notification);
	}

	pub fn take_pending(&mut self) -> Vec<Notification> {
		self.pending.drain(..).collect()
	}

	pub fn toast_manager_mut(&mut self) -> &mut ToastManager {
		&mut self.inner
	}
}
