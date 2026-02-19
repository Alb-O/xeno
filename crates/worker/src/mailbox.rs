use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::{Mutex, Notify};

/// Backpressure policy for a bounded actor mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxPolicy {
	/// Wait for capacity when full.
	Backpressure,
	/// Drop the newest message when full.
	DropNewest,
	/// Drop the oldest queued message when full.
	DropOldest,
	/// Keep only the latest message.
	LatestWins,
	/// Replace an existing queued message with the same key.
	CoalesceByKey,
}

/// Outcome from enqueueing a mailbox message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxSendOutcome {
	/// Message was enqueued without replacement.
	Enqueued,
	/// Message was dropped because policy is drop-newest and queue was full.
	DroppedNewest,
	/// Oldest queued message was replaced.
	ReplacedOldest,
	/// Existing keyed queued message was replaced.
	Coalesced,
}

/// Mailbox send error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxSendError {
	/// Mailbox is closed.
	Closed,
	/// Queue is full and non-blocking send was used.
	Full,
	/// `CoalesceByKey` requires a key extractor.
	MissingCoalesceKey,
}

struct MailboxState<T> {
	queue: VecDeque<T>,
	closed: bool,
}

struct MailboxInner<T> {
	capacity: usize,
	policy: MailboxPolicy,
	coalesce_eq: Option<Arc<dyn Fn(&T, &T) -> bool + Send + Sync>>,
	state: Mutex<MailboxState<T>>,
	notify_recv: Notify,
	notify_send: Notify,
}

/// Multi-producer actor mailbox sender.
pub struct MailboxSender<T> {
	inner: Arc<MailboxInner<T>>,
}

/// Actor mailbox receiver.
pub struct MailboxReceiver<T> {
	inner: Arc<MailboxInner<T>>,
}

/// Bounded mailbox primitive used by supervised actors.
pub struct Mailbox<T> {
	inner: Arc<MailboxInner<T>>,
}

impl<T> Clone for MailboxSender<T> {
	fn clone(&self) -> Self {
		Self {
			inner: Arc::clone(&self.inner),
		}
	}
}

impl<T> Clone for MailboxReceiver<T> {
	fn clone(&self) -> Self {
		Self {
			inner: Arc::clone(&self.inner),
		}
	}
}

impl<T> Mailbox<T> {
	/// Creates a bounded mailbox with one of the built-in policies.
	pub fn new(capacity: usize, policy: MailboxPolicy) -> Self {
		assert!(capacity > 0, "mailbox capacity must be > 0");
		Self {
			inner: Arc::new(MailboxInner {
				capacity,
				policy,
				coalesce_eq: None,
				state: Mutex::new(MailboxState {
					queue: VecDeque::with_capacity(capacity),
					closed: false,
				}),
				notify_recv: Notify::new(),
				notify_send: Notify::new(),
			}),
		}
	}

	/// Creates a bounded mailbox with key-based coalescing.
	pub fn with_coalesce_key<K>(capacity: usize, key_fn: impl Fn(&T) -> K + Send + Sync + 'static) -> Self
	where
		K: Eq + Send + Sync + 'static,
	{
		let cmp = move |lhs: &T, rhs: &T| key_fn(lhs) == key_fn(rhs);
		Self::with_coalesce_eq(capacity, cmp)
	}

	/// Creates a bounded mailbox with direct equality-based coalescing.
	pub fn with_coalesce_eq(capacity: usize, eq_fn: impl Fn(&T, &T) -> bool + Send + Sync + 'static) -> Self {
		assert!(capacity > 0, "mailbox capacity must be > 0");
		Self {
			inner: Arc::new(MailboxInner {
				capacity,
				policy: MailboxPolicy::CoalesceByKey,
				coalesce_eq: Some(Arc::new(eq_fn)),
				state: Mutex::new(MailboxState {
					queue: VecDeque::with_capacity(capacity),
					closed: false,
				}),
				notify_recv: Notify::new(),
				notify_send: Notify::new(),
			}),
		}
	}

	/// Returns a sender handle.
	pub fn sender(&self) -> MailboxSender<T> {
		MailboxSender {
			inner: Arc::clone(&self.inner),
		}
	}

	/// Returns a receiver handle.
	pub fn receiver(&self) -> MailboxReceiver<T> {
		MailboxReceiver {
			inner: Arc::clone(&self.inner),
		}
	}

	/// Returns mailbox policy.
	pub fn policy(&self) -> MailboxPolicy {
		self.inner.policy
	}
}

impl<T> MailboxSender<T> {
	/// Requests mailbox closure. Receivers drain existing items then return `None`.
	pub async fn close(&self) {
		let mut state = self.inner.state.lock().await;
		state.closed = true;
		drop(state);
		self.inner.notify_recv.notify_waiters();
		self.inner.notify_send.notify_waiters();
	}

	/// Attempts to close mailbox without waiting for a lock.
	pub fn close_now(&self) {
		if let Ok(mut state) = self.inner.state.try_lock() {
			state.closed = true;
			drop(state);
			self.inner.notify_recv.notify_waiters();
			self.inner.notify_send.notify_waiters();
		}
	}

	/// Non-blocking enqueue.
	pub async fn try_send(&self, msg: T) -> Result<MailboxSendOutcome, MailboxSendError> {
		let mut state = self.inner.state.lock().await;
		enqueue_with_policy(&self.inner, &mut state, msg)
	}

	/// Enqueue honoring policy (`Backpressure` waits for capacity).
	///
	/// For `Backpressure` policy, this loops under the lock until capacity is
	/// available, guaranteeing no silent drops. All other policies enqueue
	/// immediately according to their overflow semantics.
	pub async fn send(&self, msg: T) -> Result<MailboxSendOutcome, MailboxSendError> {
		if self.inner.policy == MailboxPolicy::Backpressure {
			loop {
				// Register the notification future *before* checking capacity
				// to avoid lost-wakeup between drop(lock) and await.
				let notified = self.inner.notify_send.notified();

				let mut state = self.inner.state.lock().await;
				if state.closed {
					return Err(MailboxSendError::Closed);
				}
				if state.queue.len() < self.inner.capacity {
					state.queue.push_back(msg);
					self.inner.notify_recv.notify_one();
					return Ok(MailboxSendOutcome::Enqueued);
				}
				drop(state);
				notified.await;
			}
		}

		let mut state = self.inner.state.lock().await;
		enqueue_with_policy(&self.inner, &mut state, msg)
	}

	/// Returns current queue length.
	pub async fn len(&self) -> usize {
		self.inner.state.lock().await.queue.len()
	}

	/// Returns queue capacity.
	pub fn capacity(&self) -> usize {
		self.inner.capacity
	}
}

impl<T> MailboxReceiver<T> {
	/// Receives one message. Returns `None` once mailbox is closed and drained.
	pub async fn recv(&self) -> Option<T> {
		loop {
			let mut state = self.inner.state.lock().await;
			if let Some(msg) = state.queue.pop_front() {
				drop(state);
				self.inner.notify_send.notify_one();
				return Some(msg);
			}
			if state.closed {
				return None;
			}
			drop(state);
			self.inner.notify_recv.notified().await;
		}
	}

	/// Returns current queue length.
	pub async fn len(&self) -> usize {
		self.inner.state.lock().await.queue.len()
	}
}

/// Non-blocking enqueue for all policies.
///
/// `Backpressure` returns `Full` when at capacity (the blocking wait lives
/// in `MailboxSender::send` which enqueues under the lock directly).
fn enqueue_with_policy<T>(inner: &MailboxInner<T>, state: &mut MailboxState<T>, msg: T) -> Result<MailboxSendOutcome, MailboxSendError> {
	if state.closed {
		return Err(MailboxSendError::Closed);
	}

	match inner.policy {
		MailboxPolicy::LatestWins => {
			let had_items = !state.queue.is_empty();
			state.queue.clear();
			state.queue.push_back(msg);
			inner.notify_recv.notify_one();
			if had_items {
				Ok(MailboxSendOutcome::Coalesced)
			} else {
				Ok(MailboxSendOutcome::Enqueued)
			}
		}
		MailboxPolicy::CoalesceByKey => {
			let Some(eq_fn) = inner.coalesce_eq.as_ref() else {
				return Err(MailboxSendError::MissingCoalesceKey);
			};
			if let Some(existing) = state.queue.iter_mut().find(|it| eq_fn(it, &msg)) {
				*existing = msg;
				inner.notify_recv.notify_one();
				return Ok(MailboxSendOutcome::Coalesced);
			}

			if state.queue.len() >= inner.capacity {
				let _ = state.queue.pop_front();
				state.queue.push_back(msg);
				inner.notify_recv.notify_one();
				Ok(MailboxSendOutcome::ReplacedOldest)
			} else {
				state.queue.push_back(msg);
				inner.notify_recv.notify_one();
				Ok(MailboxSendOutcome::Enqueued)
			}
		}
		MailboxPolicy::Backpressure => {
			if state.queue.len() < inner.capacity {
				state.queue.push_back(msg);
				inner.notify_recv.notify_one();
				Ok(MailboxSendOutcome::Enqueued)
			} else {
				Err(MailboxSendError::Full)
			}
		}
		MailboxPolicy::DropNewest => {
			if state.queue.len() < inner.capacity {
				state.queue.push_back(msg);
				inner.notify_recv.notify_one();
				Ok(MailboxSendOutcome::Enqueued)
			} else {
				Ok(MailboxSendOutcome::DroppedNewest)
			}
		}
		MailboxPolicy::DropOldest => {
			if state.queue.len() < inner.capacity {
				state.queue.push_back(msg);
				inner.notify_recv.notify_one();
				return Ok(MailboxSendOutcome::Enqueued);
			}
			let _ = state.queue.pop_front();
			state.queue.push_back(msg);
			inner.notify_recv.notify_one();
			Ok(MailboxSendOutcome::ReplacedOldest)
		}
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use std::time::Duration;

	use super::*;

	// ── A. Golden behavior tests (capacity=3, push A B C D E) ──

	#[tokio::test]
	async fn backpressure_try_send_returns_full_when_at_capacity() {
		let mailbox = Mailbox::new(3, MailboxPolicy::Backpressure);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		assert_eq!(tx.try_send(1u32).await, Ok(MailboxSendOutcome::Enqueued));
		assert_eq!(tx.try_send(2).await, Ok(MailboxSendOutcome::Enqueued));
		assert_eq!(tx.try_send(3).await, Ok(MailboxSendOutcome::Enqueued));
		// Queue full — try_send must fail with Full.
		assert_eq!(tx.try_send(4).await, Err(MailboxSendError::Full));
		assert_eq!(tx.try_send(5).await, Err(MailboxSendError::Full));

		// Drain order: FIFO, only the first 3.
		tx.close().await;
		assert_eq!(rx.recv().await, Some(1));
		assert_eq!(rx.recv().await, Some(2));
		assert_eq!(rx.recv().await, Some(3));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn backpressure_send_blocks_until_capacity_freed() {
		let mailbox = Mailbox::new(2, MailboxPolicy::Backpressure);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(1u32).await;
		let _ = tx.send(2).await;

		// send(3) should block because queue is full.
		let tx2 = tx.clone();
		let send_task = tokio::spawn(async move { tx2.send(3).await });

		// Give send_task a moment to park on the notify.
		tokio::time::sleep(Duration::from_millis(10)).await;

		// Pop one item to free capacity.
		assert_eq!(rx.recv().await, Some(1));

		// send(3) should now complete.
		let result = tokio::time::timeout(Duration::from_millis(100), send_task)
			.await
			.expect("send should unblock after pop")
			.unwrap();
		assert_eq!(result, Ok(MailboxSendOutcome::Enqueued));

		tx.close().await;
		assert_eq!(rx.recv().await, Some(2));
		assert_eq!(rx.recv().await, Some(3));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn drop_newest_rejects_incoming_when_full() {
		let mailbox = Mailbox::new(3, MailboxPolicy::DropNewest);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		assert_eq!(tx.send(1u32).await, Ok(MailboxSendOutcome::Enqueued));
		assert_eq!(tx.send(2).await, Ok(MailboxSendOutcome::Enqueued));
		assert_eq!(tx.send(3).await, Ok(MailboxSendOutcome::Enqueued));
		// Full: incoming items are dropped.
		assert_eq!(tx.send(4).await, Ok(MailboxSendOutcome::DroppedNewest));
		assert_eq!(tx.send(5).await, Ok(MailboxSendOutcome::DroppedNewest));

		tx.close().await;
		assert_eq!(rx.recv().await, Some(1));
		assert_eq!(rx.recv().await, Some(2));
		assert_eq!(rx.recv().await, Some(3));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn drop_oldest_evicts_head_when_full() {
		let mailbox = Mailbox::new(3, MailboxPolicy::DropOldest);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		assert_eq!(tx.send(1u32).await, Ok(MailboxSendOutcome::Enqueued));
		assert_eq!(tx.send(2).await, Ok(MailboxSendOutcome::Enqueued));
		assert_eq!(tx.send(3).await, Ok(MailboxSendOutcome::Enqueued));
		// Full: evicts oldest (1), then oldest (2).
		assert_eq!(tx.send(4).await, Ok(MailboxSendOutcome::ReplacedOldest));
		assert_eq!(tx.send(5).await, Ok(MailboxSendOutcome::ReplacedOldest));

		tx.close().await;
		assert_eq!(rx.recv().await, Some(3));
		assert_eq!(rx.recv().await, Some(4));
		assert_eq!(rx.recv().await, Some(5));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn latest_wins_keeps_only_the_last_sent() {
		let mailbox = Mailbox::new(8, MailboxPolicy::LatestWins);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		assert_eq!(tx.send(1u32).await, Ok(MailboxSendOutcome::Enqueued));
		assert_eq!(tx.send(2).await, Ok(MailboxSendOutcome::Coalesced));
		assert_eq!(tx.send(3).await, Ok(MailboxSendOutcome::Coalesced));
		assert_eq!(tx.send(4).await, Ok(MailboxSendOutcome::Coalesced));
		assert_eq!(tx.send(5).await, Ok(MailboxSendOutcome::Coalesced));

		tx.close().await;
		assert_eq!(rx.recv().await, Some(5));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn latest_wins_first_send_to_empty_returns_enqueued() {
		let mailbox = Mailbox::new(4, MailboxPolicy::LatestWins);
		let tx = mailbox.sender();

		assert_eq!(tx.send(42u32).await, Ok(MailboxSendOutcome::Enqueued));
	}

	// ── CoalesceByKey golden tests ──

	#[derive(Clone, Debug, PartialEq, Eq)]
	struct Msg {
		key: u64,
		value: u64,
	}

	#[tokio::test]
	async fn coalesce_replaces_in_place_preserving_order() {
		let mailbox = Mailbox::with_coalesce_key(4, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(Msg { key: 1, value: 10 }).await;
		let _ = tx.send(Msg { key: 2, value: 20 }).await;
		let _ = tx.send(Msg { key: 3, value: 30 }).await;
		// Replace key=1 in-place (position 0). Order must be [1,2,3] not [2,3,1].
		let outcome = tx.send(Msg { key: 1, value: 99 }).await;
		assert_eq!(outcome, Ok(MailboxSendOutcome::Coalesced));

		tx.close().await;
		assert_eq!(rx.recv().await, Some(Msg { key: 1, value: 99 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 2, value: 20 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 3, value: 30 }));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn coalesce_evicts_oldest_when_full_and_no_match() {
		let mailbox = Mailbox::with_coalesce_key(3, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(Msg { key: 1, value: 10 }).await;
		let _ = tx.send(Msg { key: 2, value: 20 }).await;
		let _ = tx.send(Msg { key: 3, value: 30 }).await;
		// Full, new key=4 → evicts oldest (key=1).
		let outcome = tx.send(Msg { key: 4, value: 40 }).await;
		assert_eq!(outcome, Ok(MailboxSendOutcome::ReplacedOldest));

		tx.close().await;
		assert_eq!(rx.recv().await, Some(Msg { key: 2, value: 20 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 3, value: 30 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 4, value: 40 }));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn coalesce_replaces_even_when_full() {
		let mailbox = Mailbox::with_coalesce_key(3, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(Msg { key: 1, value: 10 }).await;
		let _ = tx.send(Msg { key: 2, value: 20 }).await;
		let _ = tx.send(Msg { key: 3, value: 30 }).await;
		// Full but key=2 exists → replace in-place, no eviction.
		let outcome = tx.send(Msg { key: 2, value: 99 }).await;
		assert_eq!(outcome, Ok(MailboxSendOutcome::Coalesced));

		tx.close().await;
		assert_eq!(rx.recv().await, Some(Msg { key: 1, value: 10 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 2, value: 99 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 3, value: 30 }));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn coalesce_without_key_fn_returns_error() {
		// CoalesceByKey policy created via `new` (no key fn) must error.
		let mailbox = Mailbox::new(4, MailboxPolicy::CoalesceByKey);
		let tx = mailbox.sender();

		assert_eq!(tx.send(1u32).await, Err(MailboxSendError::MissingCoalesceKey));
	}

	// ── Closed mailbox tests ──

	#[tokio::test]
	async fn send_on_closed_mailbox_returns_closed() {
		let mailbox = Mailbox::new(4, MailboxPolicy::Backpressure);
		let tx = mailbox.sender();
		tx.close().await;

		assert_eq!(tx.send(1u32).await, Err(MailboxSendError::Closed));
		assert_eq!(tx.try_send(2).await, Err(MailboxSendError::Closed));
	}

	#[tokio::test]
	async fn recv_drains_then_returns_none_on_close() {
		let mailbox = Mailbox::new(4, MailboxPolicy::DropNewest);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(10u32).await;
		let _ = tx.send(20).await;
		tx.close().await;

		assert_eq!(rx.recv().await, Some(10));
		assert_eq!(rx.recv().await, Some(20));
		assert_eq!(rx.recv().await, None);
		// Repeated recv after drain still returns None.
		assert_eq!(rx.recv().await, None);
	}

	// ── Interleaved push/pop tests ──

	#[tokio::test]
	async fn drop_oldest_interleaved_push_pop() {
		let mailbox = Mailbox::new(2, MailboxPolicy::DropOldest);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(1u32).await;
		let _ = tx.send(2).await;
		assert_eq!(rx.recv().await, Some(1));

		// Queue: [2]. Push 3,4 — at push(4) queue is [2,3], evicts 2.
		let _ = tx.send(3).await;
		let _ = tx.send(4).await;

		tx.close().await;
		assert_eq!(rx.recv().await, Some(3));
		assert_eq!(rx.recv().await, Some(4));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn coalesce_interleaved_push_pop_preserves_order() {
		let mailbox = Mailbox::with_coalesce_key(3, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(Msg { key: 1, value: 10 }).await;
		let _ = tx.send(Msg { key: 2, value: 20 }).await;
		assert_eq!(rx.recv().await, Some(Msg { key: 1, value: 10 }));

		// Queue: [key=2]. Push key=2 again (coalesce), then key=3.
		let outcome = tx.send(Msg { key: 2, value: 99 }).await;
		assert_eq!(outcome, Ok(MailboxSendOutcome::Coalesced));
		let _ = tx.send(Msg { key: 3, value: 30 }).await;

		tx.close().await;
		assert_eq!(rx.recv().await, Some(Msg { key: 2, value: 99 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 3, value: 30 }));
		assert_eq!(rx.recv().await, None);
	}

	// ── B. Invariant stress tests (deterministic xorshift) ──

	/// Deterministic pseudo-random number generator for reproducible stress tests.
	struct Xorshift64(u64);

	impl Xorshift64 {
		fn new(seed: u64) -> Self {
			Self(seed)
		}

		fn next(&mut self) -> u64 {
			let mut x = self.0;
			x ^= x << 13;
			x ^= x >> 7;
			x ^= x << 17;
			self.0 = x;
			x
		}

		fn next_usize(&mut self, bound: usize) -> usize {
			(self.next() % bound as u64) as usize
		}
	}

	/// Reference model for a bounded queue with a given policy.
	struct QueueModel {
		capacity: usize,
		policy: MailboxPolicy,
		queue: std::collections::VecDeque<u32>,
	}

	impl QueueModel {
		fn new(capacity: usize, policy: MailboxPolicy) -> Self {
			Self {
				capacity,
				policy,
				queue: std::collections::VecDeque::with_capacity(capacity),
			}
		}

		fn push(&mut self, val: u32) -> Result<MailboxSendOutcome, MailboxSendError> {
			match self.policy {
				MailboxPolicy::Backpressure => {
					if self.queue.len() < self.capacity {
						self.queue.push_back(val);
						Ok(MailboxSendOutcome::Enqueued)
					} else {
						Err(MailboxSendError::Full)
					}
				}
				MailboxPolicy::DropNewest => {
					if self.queue.len() < self.capacity {
						self.queue.push_back(val);
						Ok(MailboxSendOutcome::Enqueued)
					} else {
						Ok(MailboxSendOutcome::DroppedNewest)
					}
				}
				MailboxPolicy::DropOldest => {
					if self.queue.len() < self.capacity {
						self.queue.push_back(val);
						Ok(MailboxSendOutcome::Enqueued)
					} else {
						let _ = self.queue.pop_front();
						self.queue.push_back(val);
						Ok(MailboxSendOutcome::ReplacedOldest)
					}
				}
				MailboxPolicy::LatestWins => {
					let had_items = !self.queue.is_empty();
					self.queue.clear();
					self.queue.push_back(val);
					if had_items {
						Ok(MailboxSendOutcome::Coalesced)
					} else {
						Ok(MailboxSendOutcome::Enqueued)
					}
				}
				MailboxPolicy::CoalesceByKey => unreachable!("use keyed model"),
			}
		}

		fn pop(&mut self) -> Option<u32> {
			self.queue.pop_front()
		}

		fn contents(&self) -> Vec<u32> {
			self.queue.iter().copied().collect()
		}
	}

	/// Reference model for CoalesceByKey using (key, value) pairs.
	struct KeyedQueueModel {
		capacity: usize,
		queue: std::collections::VecDeque<(u64, u32)>,
	}

	impl KeyedQueueModel {
		fn new(capacity: usize) -> Self {
			Self {
				capacity,
				queue: std::collections::VecDeque::with_capacity(capacity),
			}
		}

		fn push(&mut self, key: u64, value: u32) -> Result<MailboxSendOutcome, MailboxSendError> {
			// Replace in-place if key exists.
			if let Some(existing) = self.queue.iter_mut().find(|(k, _)| *k == key) {
				existing.1 = value;
				return Ok(MailboxSendOutcome::Coalesced);
			}
			if self.queue.len() >= self.capacity {
				let _ = self.queue.pop_front();
				self.queue.push_back((key, value));
				Ok(MailboxSendOutcome::ReplacedOldest)
			} else {
				self.queue.push_back((key, value));
				Ok(MailboxSendOutcome::Enqueued)
			}
		}

		fn pop(&mut self) -> Option<(u64, u32)> {
			self.queue.pop_front()
		}

		fn contents(&self) -> Vec<(u64, u32)> {
			self.queue.iter().copied().collect()
		}
	}

	/// Collects all items from a mailbox receiver (mailbox must be closed first).
	async fn drain_all<T>(rx: &MailboxReceiver<T>) -> Vec<T> {
		let mut items = Vec::new();
		while let Some(item) = rx.recv().await {
			items.push(item);
		}
		items
	}

	#[tokio::test]
	async fn stress_backpressure_matches_model() {
		const OPS: usize = 5_000;
		let capacity = 4;
		let mailbox = Mailbox::new(capacity, MailboxPolicy::Backpressure);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();
		let mut model = QueueModel::new(capacity, MailboxPolicy::Backpressure);
		let mut rng = Xorshift64::new(0xDEAD_BEEF);

		for i in 0..OPS {
			// 60% push, 40% pop.
			if rng.next_usize(10) < 6 {
				let val = i as u32;
				let real = tx.try_send(val).await;
				let expected = model.push(val);
				assert_eq!(real, expected, "op {i}: push({val})");
			} else {
				let real = tokio::time::timeout(Duration::from_millis(1), rx.recv()).await;
				let expected = model.pop();
				match (real, expected) {
					(Ok(r), e) => assert_eq!(r, e, "op {i}: pop"),
					(Err(_), None) => {} // Both empty, recv timed out.
					(Err(_), Some(v)) => panic!("op {i}: model has {v} but recv timed out"),
				}
			}
		}

		tx.close().await;
		let remaining = drain_all(&rx).await;
		assert_eq!(remaining, model.contents(), "final drain mismatch");
	}

	#[tokio::test]
	async fn stress_drop_newest_matches_model() {
		const OPS: usize = 10_000;
		let capacity = 4;
		let mailbox = Mailbox::new(capacity, MailboxPolicy::DropNewest);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();
		let mut model = QueueModel::new(capacity, MailboxPolicy::DropNewest);
		let mut rng = Xorshift64::new(0xCAFE_BABE);

		for i in 0..OPS {
			if rng.next_usize(10) < 6 {
				let val = i as u32;
				let real = tx.send(val).await;
				let expected = model.push(val);
				assert_eq!(real, expected, "op {i}: push({val})");
			} else {
				let real = tokio::time::timeout(Duration::from_millis(1), rx.recv()).await;
				let expected = model.pop();
				match (real, expected) {
					(Ok(r), e) => assert_eq!(r, e, "op {i}: pop"),
					(Err(_), None) => {}
					(Err(_), Some(v)) => panic!("op {i}: model has {v} but recv timed out"),
				}
			}
		}

		tx.close().await;
		let remaining = drain_all(&rx).await;
		assert_eq!(remaining, model.contents(), "final drain mismatch");
	}

	#[tokio::test]
	async fn stress_drop_oldest_matches_model() {
		const OPS: usize = 10_000;
		let capacity = 4;
		let mailbox = Mailbox::new(capacity, MailboxPolicy::DropOldest);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();
		let mut model = QueueModel::new(capacity, MailboxPolicy::DropOldest);
		let mut rng = Xorshift64::new(0x1234_5678);

		for i in 0..OPS {
			if rng.next_usize(10) < 6 {
				let val = i as u32;
				let real = tx.send(val).await;
				let expected = model.push(val);
				assert_eq!(real, expected, "op {i}: push({val})");
			} else {
				let real = tokio::time::timeout(Duration::from_millis(1), rx.recv()).await;
				let expected = model.pop();
				match (real, expected) {
					(Ok(r), e) => assert_eq!(r, e, "op {i}: pop"),
					(Err(_), None) => {}
					(Err(_), Some(v)) => panic!("op {i}: model has {v} but recv timed out"),
				}
			}
		}

		tx.close().await;
		let remaining = drain_all(&rx).await;
		assert_eq!(remaining, model.contents(), "final drain mismatch");
	}

	#[tokio::test]
	async fn stress_latest_wins_matches_model() {
		const OPS: usize = 10_000;
		let capacity = 4;
		let mailbox = Mailbox::new(capacity, MailboxPolicy::LatestWins);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();
		let mut model = QueueModel::new(capacity, MailboxPolicy::LatestWins);
		let mut rng = Xorshift64::new(0xABCD_EF01);

		for i in 0..OPS {
			if rng.next_usize(10) < 6 {
				let val = i as u32;
				let real = tx.send(val).await;
				let expected = model.push(val);
				assert_eq!(real, expected, "op {i}: push({val})");
			} else {
				let real = tokio::time::timeout(Duration::from_millis(1), rx.recv()).await;
				let expected = model.pop();
				match (real, expected) {
					(Ok(r), e) => assert_eq!(r, e, "op {i}: pop"),
					(Err(_), None) => {}
					(Err(_), Some(v)) => panic!("op {i}: model has {v} but recv timed out"),
				}
			}
		}

		tx.close().await;
		let remaining = drain_all(&rx).await;
		assert_eq!(remaining, model.contents(), "final drain mismatch");
	}

	#[tokio::test]
	async fn stress_coalesce_by_key_matches_model() {
		const OPS: usize = 10_000;
		let capacity = 4;
		let mailbox = Mailbox::with_coalesce_key(capacity, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();
		let mut model = KeyedQueueModel::new(capacity);
		let mut rng = Xorshift64::new(0xFEED_FACE);
		// Use a small key space to force frequent coalescing.
		let key_space = 6u64;

		for i in 0..OPS {
			if rng.next_usize(10) < 6 {
				let key = rng.next() % key_space;
				let val = i as u32;
				let real = tx.send(Msg { key, value: val as u64 }).await;
				let expected = model.push(key, val);
				assert_eq!(real, expected, "op {i}: push(key={key}, val={val})");
			} else {
				let real = tokio::time::timeout(Duration::from_millis(1), rx.recv()).await;
				let expected = model.pop();
				match (real, expected) {
					(Ok(Some(msg)), Some((k, v))) => {
						assert_eq!(msg.key, k, "op {i}: pop key");
						assert_eq!(msg.value, v as u64, "op {i}: pop value");
					}
					(Ok(None), None) => {}
					(Err(_), None) => {}
					(real, expected) => panic!("op {i}: pop mismatch: real={real:?}, expected={expected:?}"),
				}
			}
		}

		tx.close().await;
		let remaining: Vec<_> = drain_all(&rx).await.into_iter().map(|m| (m.key, m.value as u32)).collect();
		assert_eq!(remaining, model.contents(), "final drain mismatch");
	}

	// ── Edge cases ──

	#[tokio::test]
	async fn empty_mailbox_recv_blocks_until_send() {
		let mailbox = Mailbox::new(4, MailboxPolicy::Backpressure);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		// recv on empty should block, not return None.
		let recv_timeout = tokio::time::timeout(Duration::from_millis(20), rx.recv()).await;
		assert!(recv_timeout.is_err(), "recv on empty should block");

		let _ = tx.send(42u32).await;
		assert_eq!(rx.recv().await, Some(42));
	}

	#[tokio::test]
	async fn len_tracks_queue_depth() {
		let mailbox = Mailbox::new(4, MailboxPolicy::DropNewest);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		assert_eq!(tx.len().await, 0);
		assert_eq!(rx.len().await, 0);

		let _ = tx.send(1u32).await;
		let _ = tx.send(2).await;
		assert_eq!(tx.len().await, 2);
		assert_eq!(rx.len().await, 2);

		let _ = rx.recv().await;
		assert_eq!(tx.len().await, 1);
	}

	#[tokio::test]
	async fn close_now_is_best_effort() {
		let mailbox = Mailbox::new(4, MailboxPolicy::Backpressure);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(1u32).await;
		tx.close_now();

		// After close_now, remaining items drain then None.
		assert_eq!(rx.recv().await, Some(1));
		assert_eq!(rx.recv().await, None);
	}

	// ── Backpressure concurrency: no silent drops ──

	#[tokio::test]
	async fn backpressure_multi_sender_never_drops() {
		const SENDERS: usize = 8;
		const ITEMS_PER_SENDER: usize = 200;
		let total = SENDERS * ITEMS_PER_SENDER;

		let mailbox = Mailbox::new(2, MailboxPolicy::Backpressure);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let barrier = Arc::new(tokio::sync::Barrier::new(SENDERS));

		let mut handles = Vec::new();
		for sender_id in 0..SENDERS {
			let tx = tx.clone();
			let barrier = Arc::clone(&barrier);
			handles.push(tokio::spawn(async move {
				// All senders stampede at once.
				barrier.wait().await;
				for seq in 0..ITEMS_PER_SENDER {
					let val = (sender_id * ITEMS_PER_SENDER + seq) as u32;
					let outcome = tx.send(val).await;
					assert_eq!(outcome, Ok(MailboxSendOutcome::Enqueued), "sender {sender_id} seq {seq}: must not drop");
				}
			}));
		}

		// Drain receiver until we have all items.
		let receiver = tokio::spawn(async move {
			let mut received = Vec::with_capacity(total);
			for _ in 0..total {
				let val = rx.recv().await.expect("should not close early");
				received.push(val);
			}
			received
		});

		for h in handles {
			h.await.unwrap();
		}
		tx.close().await;

		let received = receiver.await.unwrap();
		assert_eq!(received.len(), total, "must receive exactly N*M items");

		// Verify every value was delivered exactly once.
		let mut sorted = received;
		sorted.sort();
		let expected: Vec<u32> = (0..total as u32).collect();
		assert_eq!(sorted, expected, "all items delivered without loss or duplication");
	}

	#[tokio::test]
	async fn backpressure_send_returns_closed_when_closed_while_waiting() {
		let mailbox = Mailbox::new(1, MailboxPolicy::Backpressure);
		let tx = mailbox.sender();
		let _rx = mailbox.receiver();

		// Fill the single slot.
		let _ = tx.send(1u32).await;

		// send(2) will block because queue is full.
		let tx2 = tx.clone();
		let send_task = tokio::spawn(async move { tx2.send(2).await });

		tokio::time::sleep(Duration::from_millis(10)).await;

		// Close the mailbox while sender is blocked.
		tx.close().await;

		let result = tokio::time::timeout(Duration::from_millis(100), send_task)
			.await
			.expect("blocked send should wake on close")
			.unwrap();
		assert_eq!(result, Err(MailboxSendError::Closed));
	}
}
