//! Toast manager for handling multiple notifications.

use std::collections::HashMap;
use std::time::Duration;

mod layout;
mod render;
mod state;

use state::ToastState;

use super::toast::Toast;
use super::types::{Anchor, Overflow};
use crate::buffer::Buffer;
use crate::layout::Rect;

/// Manages multiple toast notifications with lifecycle, animations, and stacking.
#[derive(Debug)]
pub struct ToastManager {
	/// Active toast states keyed by ID.
	states: HashMap<u64, ToastState>,
	/// Next ID to assign to a new toast.
	next_id: u64,
	/// Maximum number of visible toasts per anchor (None = unlimited).
	max_visible: Option<usize>,
	/// Behavior when max_visible is exceeded.
	overflow: Overflow,
}

impl Default for ToastManager {
	fn default() -> Self {
		Self::new()
	}
}

impl ToastManager {
	/// Creates a new empty toast manager.
	pub fn new() -> Self {
		Self {
			states: HashMap::new(),
			next_id: 0,
			max_visible: None,
			overflow: Overflow::default(),
		}
	}

	/// Sets the maximum number of visible toasts.
	#[must_use]
	pub fn max_visible(mut self, max: Option<usize>) -> Self {
		self.max_visible = max;
		self
	}

	/// Sets the overflow behavior when the limit is reached.
	#[must_use]
	pub fn overflow(mut self, overflow: Overflow) -> Self {
		self.overflow = overflow;
		self
	}

	/// Adds a toast and returns its ID.
	///
	/// If a toast with identical content and anchor already exists (and is not
	/// exiting), increments its stack count and resets the dismiss timer.
	pub fn push(&mut self, toast: Toast) -> u64 {
		if let Some((&id, state)) = self.states.iter_mut().find(|(_, s)| {
			s.can_stack() && s.toast.anchor == toast.anchor && s.toast.content == toast.content
		}) {
			state.increment_stack();
			return id;
		}

		let id = self.next_id;
		self.next_id = self.next_id.wrapping_add(1);

		if let Some(max) = self.max_visible {
			while self.states.len() >= max {
				let to_remove = match self.overflow {
					Overflow::DropOldest => self.oldest_id(),
					Overflow::DropNewest => self.newest_id(),
				};
				if let Some(remove_id) = to_remove {
					self.states.remove(&remove_id);
				} else {
					break;
				}
			}
		}

		self.states.insert(id, ToastState::new(toast));
		id
	}

	/// Removes a toast by ID. Returns true if it existed.
	pub fn remove(&mut self, id: u64) -> bool {
		self.states.remove(&id).is_some()
	}

	/// Clears all toasts.
	pub fn clear(&mut self) {
		self.states.clear();
	}

	/// Returns true if there are no toasts.
	pub fn is_empty(&self) -> bool {
		self.states.is_empty()
	}

	/// Returns the number of active toasts.
	pub fn len(&self) -> usize {
		self.states.len()
	}

	/// Advances all toast animations and removes finished toasts.
	pub fn tick(&mut self, delta: Duration) {
		for state in self.states.values_mut() {
			state.update(delta);
		}
		self.states.retain(|_, state| !state.is_finished());
	}

	/// Renders all toasts to the buffer.
	pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
		if self.states.is_empty() {
			return;
		}

		let mut by_anchor: HashMap<Anchor, Vec<u64>> = HashMap::new();
		for (&id, state) in &self.states {
			by_anchor.entry(state.toast.anchor).or_default().push(id);
		}

		for (anchor, ids) in by_anchor {
			self.render_anchor_group(anchor, &ids, area, buf);
		}
	}

	/// Returns the ID of the oldest toast.
	fn oldest_id(&self) -> Option<u64> {
		self.states
			.iter()
			.min_by_key(|(_, s)| s.created_at)
			.map(|(&id, _)| id)
	}

	/// Returns the ID of the newest toast.
	fn newest_id(&self) -> Option<u64> {
		self.states
			.iter()
			.max_by_key(|(_, s)| s.created_at)
			.map(|(&id, _)| id)
	}
}
