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
	coalesce_key: Option<Arc<dyn Fn(&T) -> u64 + Send + Sync>>,
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
				coalesce_key: None,
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
	pub fn with_coalesce_key(capacity: usize, key_fn: impl Fn(&T) -> u64 + Send + Sync + 'static) -> Self {
		assert!(capacity > 0, "mailbox capacity must be > 0");
		Self {
			inner: Arc::new(MailboxInner {
				capacity,
				policy: MailboxPolicy::CoalesceByKey,
				coalesce_key: Some(Arc::new(key_fn)),
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
		enqueue_with_policy(&self.inner, &mut state, msg, false)
	}

	/// Enqueue honoring policy (`Backpressure` waits for capacity).
	pub async fn send(&self, msg: T) -> Result<MailboxSendOutcome, MailboxSendError> {
		if self.inner.policy == MailboxPolicy::Backpressure {
			loop {
				let state = self.inner.state.lock().await;
				if state.closed {
					return Err(MailboxSendError::Closed);
				}
				if state.queue.len() < self.inner.capacity {
					drop(state);
					break;
				}
				drop(state);
				self.inner.notify_send.notified().await;
			}
		}

		let mut state = self.inner.state.lock().await;
		enqueue_with_policy(&self.inner, &mut state, msg, true)
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

fn enqueue_with_policy<T>(inner: &MailboxInner<T>, state: &mut MailboxState<T>, msg: T, blocking: bool) -> Result<MailboxSendOutcome, MailboxSendError> {
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
			let Some(key_fn) = inner.coalesce_key.as_ref() else {
				return Err(MailboxSendError::MissingCoalesceKey);
			};
			let key = key_fn(&msg);
			if let Some(existing) = state.queue.iter_mut().find(|it| key_fn(it) == key) {
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
				return Ok(MailboxSendOutcome::Enqueued);
			}
			if blocking {
				Ok(MailboxSendOutcome::DroppedNewest)
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
	use super::*;

	#[tokio::test]
	async fn latest_wins_keeps_single_latest_value() {
		let mailbox = Mailbox::new(8, MailboxPolicy::LatestWins);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(1u32).await;
		let _ = tx.send(2u32).await;
		let _ = tx.send(3u32).await;
		tx.close().await;

		assert_eq!(rx.recv().await, Some(3));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn coalesce_replaces_matching_key() {
		#[derive(Clone, Debug, PartialEq, Eq)]
		struct Msg {
			key: u64,
			value: u64,
		}

		let mailbox = Mailbox::with_coalesce_key(4, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(Msg { key: 1, value: 10 }).await;
		let _ = tx.send(Msg { key: 2, value: 20 }).await;
		let _ = tx.send(Msg { key: 1, value: 99 }).await;
		tx.close().await;

		assert_eq!(rx.recv().await, Some(Msg { key: 1, value: 99 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 2, value: 20 }));
		assert_eq!(rx.recv().await, None);
	}
}
