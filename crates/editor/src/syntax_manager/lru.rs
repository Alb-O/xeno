use std::collections::VecDeque;

use rustc_hash::FxHashSet;

use crate::buffer::DocumentId;

/// Bounded LRU tracker for recently visible documents.
///
/// Maintains an ordered set of up to `capacity` document IDs, with most-recently-touched
/// at the front. Used by the syntax scheduler to promote recently-hidden documents to
/// [`SyntaxHotness::Warm`](super::SyntaxHotness::Warm), preventing immediate retention
/// drops that would cause highlight flashes when switching buffers.
///
/// Internally uses a `VecDeque` for ordering and an `FxHashSet` for O(1) membership
/// checks. Touch is O(n) in the capacity (linear scan for position removal), which is
/// acceptable for the expected capacity (~32).
#[derive(Debug, Clone)]
pub struct RecentDocLru {
	capacity: usize,
	order: VecDeque<DocumentId>,
	set: FxHashSet<DocumentId>,
}

impl RecentDocLru {
	/// Creates a new LRU with the given capacity (minimum 1).
	pub fn new(capacity: usize) -> Self {
		let capacity = capacity.max(1);
		Self {
			capacity,
			order: VecDeque::with_capacity(capacity),
			set: FxHashSet::default(),
		}
	}

	/// Marks `doc_id` as recently visible. If already present, moves it to the front.
	/// If at capacity, evicts the least-recently-touched entry.
	pub fn touch(&mut self, doc_id: DocumentId) {
		if self.set.contains(&doc_id) {
			if let Some(pos) = self.order.iter().position(|&id| id == doc_id) {
				self.order.remove(pos);
			}
		} else {
			if self.order.len() >= self.capacity
				&& let Some(oldest) = self.order.pop_back()
			{
				self.set.remove(&oldest);
			}
			self.set.insert(doc_id);
		}
		self.order.push_front(doc_id);
	}

	/// Returns `true` if `doc_id` is tracked (was recently visible).
	pub fn contains(&self, doc_id: DocumentId) -> bool {
		self.set.contains(&doc_id)
	}

	/// Explicitly removes `doc_id` (e.g. on document close).
	pub fn remove(&mut self, doc_id: DocumentId) {
		if self.set.remove(&doc_id)
			&& let Some(pos) = self.order.iter().position(|&id| id == doc_id)
		{
			self.order.remove(pos);
		}
	}
}

impl Default for RecentDocLru {
	/// Default capacity of 32 documents.
	fn default() -> Self {
		Self::new(32)
	}
}
