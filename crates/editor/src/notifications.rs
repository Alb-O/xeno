//! Editor notification center wrapper.
//!
//! Owns typed notification queueing for frontend presentation layers.
//!
//! Frontend crates are responsible for toast lifecycle state, visual mapping,
//! and rendering.

use std::collections::VecDeque;

use xeno_registry::notifications::Notification;

pub(crate) struct NotificationCenter {
	pending: VecDeque<Notification>,
	clear_epoch: u64,
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

	pub(crate) fn clear_epoch(&self) -> u64 {
		self.clear_epoch
	}
}
