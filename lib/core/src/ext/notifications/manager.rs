use std::collections::HashMap;
use std::time::Duration;

use ratatui::prelude::{Frame, Rect};

use crate::ext::notifications::notification::Notification;
use crate::ext::notifications::render::render_notifications;
use crate::ext::notifications::state::{ManagerDefaults, NotificationState};
use crate::ext::notifications::types::{Anchor, NotificationError, Overflow};

#[derive(Debug)]
pub struct Notifications {
	states: HashMap<u64, NotificationState>,
	by_anchor: HashMap<Anchor, Vec<u64>>,
	next_id: u64,
	defaults: ManagerDefaults,
	max_concurrent: Option<usize>,
	overflow: Overflow,
}

impl Notifications {
	pub fn new() -> Self {
		Self {
			states: HashMap::new(),
			by_anchor: HashMap::new(),
			next_id: 0,
			defaults: ManagerDefaults::default(),
			max_concurrent: None,
			overflow: Overflow::default(),
		}
	}

	pub fn max_concurrent(mut self, max: Option<usize>) -> Self {
		self.max_concurrent = max;
		self
	}

	pub fn overflow(mut self, behavior: Overflow) -> Self {
		self.overflow = behavior;
		self
	}

	pub fn add(&mut self, notification: Notification) -> Result<u64, NotificationError> {
		let id = self.next_id;
		self.next_id = self.next_id.checked_add(1).unwrap_or(0);

		let anchor = notification.anchor;
		self.enforce_limit(anchor);

		let state = NotificationState::new(id, notification, &self.defaults);
		self.states.insert(id, state);
		self.by_anchor.entry(anchor).or_default().push(id);

		Ok(id)
	}

	pub fn remove(&mut self, id: u64) -> bool {
		if let Some(state) = self.states.remove(&id) {
			let anchor = state.notification.anchor;
			if let Some(ids) = self.by_anchor.get_mut(&anchor) {
				ids.retain(|&existing_id| existing_id != id);
			}
			true
		} else {
			false
		}
	}

	pub fn clear(&mut self) {
		self.states.clear();
		self.by_anchor.clear();
	}

	pub fn tick(&mut self, delta: Duration) {
		let states_to_update: Vec<u64> = self.states.keys().copied().collect();

		for id in states_to_update {
			if let Some(state) = self.states.get_mut(&id) {
				state.update(delta);
			}
		}

		let finished: Vec<u64> = self
			.states
			.iter()
			.filter_map(|(id, state)| {
				if state.current_phase == crate::ext::notifications::types::AnimationPhase::Finished
				{
					Some(*id)
				} else {
					None
				}
			})
			.collect();

		for id in finished {
			self.remove(id);
		}
	}

	pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
		render_notifications(
			&mut self.states,
			&self.by_anchor,
			frame,
			area,
			self.max_concurrent,
		);
	}

	fn enforce_limit(&mut self, anchor: Anchor) {
		if let Some(max) = self.max_concurrent {
			let current_count = self.by_anchor.get(&anchor).map_or(0, |ids| ids.len());

			if current_count >= max {
				let id_to_remove = match self.overflow {
					Overflow::DiscardOldest => self.find_oldest_at_anchor(anchor),
					Overflow::DiscardNewest => self.find_newest_at_anchor(anchor),
				};

				if let Some(id) = id_to_remove {
					self.remove(id);
				}
			}
		}
	}

	fn find_oldest_at_anchor(&self, anchor: Anchor) -> Option<u64> {
		self.by_anchor
			.get(&anchor)?
			.iter()
			.filter_map(|id| self.states.get(id).map(|state| (id, state.created_at)))
			.min_by_key(|&(_, created_at)| created_at)
			.map(|(&id, _)| id)
	}

	fn find_newest_at_anchor(&self, anchor: Anchor) -> Option<u64> {
		self.by_anchor
			.get(&anchor)?
			.iter()
			.filter_map(|id| self.states.get(id).map(|state| (id, state.created_at)))
			.max_by_key(|&(_, created_at)| created_at)
			.map(|(&id, _)| id)
	}
}

impl Default for Notifications {
	fn default() -> Self {
		Self::new()
	}
}
