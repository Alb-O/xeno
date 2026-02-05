use std::collections::VecDeque;

use rustc_hash::FxHashSet;

use crate::buffer::DocumentId;

/// Manual LRU for tracking recently visible documents.
///
/// Used to promote documents to `Warm` hotness state, preventing
/// immediate retention drops when they are hidden.
#[derive(Debug, Clone)]
pub struct RecentDocLru {
	capacity: usize,
	order: VecDeque<DocumentId>,
	set: FxHashSet<DocumentId>,
}

impl RecentDocLru {
	pub fn new(capacity: usize) -> Self {
		let capacity = capacity.max(1);
		Self {
			capacity,
			order: VecDeque::with_capacity(capacity),
			set: FxHashSet::default(),
		}
	}

	pub fn touch(&mut self, doc_id: DocumentId) {
		if self.set.contains(&doc_id) {
			if let Some(pos) = self.order.iter().position(|&id| id == doc_id) {
				self.order.remove(pos);
			}
		} else {
			if self.order.len() >= self.capacity
				&& let Some(oldest) = self.order.pop_back() {
					self.set.remove(&oldest);
				}
			self.set.insert(doc_id);
		}
		self.order.push_front(doc_id);
	}

	pub fn contains(&self, doc_id: DocumentId) -> bool {
		self.set.contains(&doc_id)
	}

	pub fn remove(&mut self, doc_id: DocumentId) {
		if self.set.remove(&doc_id)
			&& let Some(pos) = self.order.iter().position(|&id| id == doc_id) {
				self.order.remove(pos);
			}
	}
}

impl Default for RecentDocLru {
	fn default() -> Self {
		Self::new(32)
	}
}
