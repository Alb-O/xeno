use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::{Mutex, Notify};

/// Mailbox send error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxSendError {
	/// Mailbox is closed.
	Closed,
}

struct MailboxState<T> {
	queue: VecDeque<T>,
	closed: bool,
}

type CoalesceEqFn<T> = dyn Fn(&T, &T) -> bool + Send + Sync;

/// Mode-specific data. Each variant owns only what it needs.
enum MailboxPolicy<T> {
	/// Senders wait for capacity. `notify_send` wakes blocked senders on pop.
	Backpressure { notify_send: Notify },
	/// Replace matching key in-place, or evict oldest when full.
	CoalesceByKey { coalesce_eq: Arc<CoalesceEqFn<T>> },
}

struct MailboxInner<T> {
	capacity: usize,
	state: Mutex<MailboxState<T>>,
	notify_recv: Notify,
	policy: MailboxPolicy<T>,
}

/// Multi-producer actor mailbox sender.
pub(crate) struct MailboxSender<T> {
	inner: Arc<MailboxInner<T>>,
}

/// Actor mailbox receiver.
pub(crate) struct MailboxReceiver<T> {
	inner: Arc<MailboxInner<T>>,
}

/// Bounded mailbox primitive used by supervised actors.
pub(crate) struct Mailbox<T> {
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
	/// Creates a bounded mailbox with backpressure (senders wait when full).
	pub fn backpressure(capacity: usize) -> Self {
		assert!(capacity > 0, "mailbox capacity must be > 0");
		Self {
			inner: Arc::new(MailboxInner {
				capacity,
				state: Mutex::new(MailboxState {
					queue: VecDeque::with_capacity(capacity),
					closed: false,
				}),
				notify_recv: Notify::new(),
				policy: MailboxPolicy::Backpressure {
					notify_send: Notify::new(),
				},
			}),
		}
	}

	/// Creates a bounded mailbox with key-based coalescing.
	///
	/// Messages with matching keys replace the existing entry in-place.
	/// When full and no key match exists, the oldest entry is evicted.
	pub fn coalesce_by_key<K>(capacity: usize, key_fn: impl Fn(&T) -> K + Send + Sync + 'static) -> Self
	where
		K: Eq + Send + Sync + 'static,
	{
		let cmp = move |lhs: &T, rhs: &T| key_fn(lhs) == key_fn(rhs);
		Self::coalesce_by_eq(capacity, cmp)
	}

	/// Creates a bounded mailbox with direct equality-based coalescing.
	pub fn coalesce_by_eq(capacity: usize, eq_fn: impl Fn(&T, &T) -> bool + Send + Sync + 'static) -> Self {
		assert!(capacity > 0, "mailbox capacity must be > 0");
		Self {
			inner: Arc::new(MailboxInner {
				capacity,
				state: Mutex::new(MailboxState {
					queue: VecDeque::with_capacity(capacity),
					closed: false,
				}),
				notify_recv: Notify::new(),
				policy: MailboxPolicy::CoalesceByKey {
					coalesce_eq: Arc::new(eq_fn),
				},
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
}

impl<T> MailboxSender<T> {
	/// Requests mailbox closure. Receivers drain existing items then return `None`.
	pub async fn close(&self) {
		let mut state = self.inner.state.lock().await;
		state.closed = true;
		drop(state);
		self.inner.notify_recv.notify_waiters();
		if let MailboxPolicy::Backpressure { notify_send } = &self.inner.policy {
			notify_send.notify_waiters();
		}
	}

	/// Attempts to close mailbox without waiting for a lock.
	pub fn close_now(&self) {
		if let Ok(mut state) = self.inner.state.try_lock() {
			state.closed = true;
			drop(state);
			self.inner.notify_recv.notify_waiters();
			if let MailboxPolicy::Backpressure { notify_send } = &self.inner.policy {
				notify_send.notify_waiters();
			}
		}
	}

	/// Enqueue honoring policy.
	///
	/// `Backpressure` waits for capacity, guaranteeing no silent drops.
	/// `CoalesceByKey` replaces in-place or evicts oldest; never blocks.
	pub async fn send(&self, msg: T) -> Result<(), MailboxSendError> {
		match &self.inner.policy {
			MailboxPolicy::Backpressure { notify_send } => {
				loop {
					let notified = notify_send.notified();

					let mut state = self.inner.state.lock().await;
					if state.closed {
						return Err(MailboxSendError::Closed);
					}
					if state.queue.len() < self.inner.capacity {
						state.queue.push_back(msg);
						self.inner.notify_recv.notify_one();
						return Ok(());
					}
					drop(state);
					notified.await;
				}
			}
			MailboxPolicy::CoalesceByKey { coalesce_eq } => {
				let mut state = self.inner.state.lock().await;
				if state.closed {
					return Err(MailboxSendError::Closed);
				}

				if let Some(existing) = state.queue.iter_mut().find(|it| coalesce_eq(it, &msg)) {
					*existing = msg;
					self.inner.notify_recv.notify_one();
					return Ok(());
				}

				if state.queue.len() >= self.inner.capacity {
					let _ = state.queue.pop_front();
				}
				state.queue.push_back(msg);
				self.inner.notify_recv.notify_one();
				Ok(())
			}
		}
	}

}

impl<T> MailboxReceiver<T> {
	/// Receives one message. Returns `None` once mailbox is closed and drained.
	pub async fn recv(&self) -> Option<T> {
		loop {
			let mut state = self.inner.state.lock().await;
			if let Some(msg) = state.queue.pop_front() {
				drop(state);
				if let MailboxPolicy::Backpressure { notify_send } = &self.inner.policy {
					notify_send.notify_one();
				}
				return Some(msg);
			}
			if state.closed {
				return None;
			}
			drop(state);
			self.inner.notify_recv.notified().await;
		}
	}

}

#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use std::time::Duration;

	use super::*;

	// ── Backpressure golden tests ──

	#[tokio::test]
	async fn backpressure_send_blocks_until_capacity_freed() {
		let mailbox = Mailbox::backpressure(2);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(1u32).await;
		let _ = tx.send(2).await;

		let tx2 = tx.clone();
		let send_task = crate::spawn::spawn(crate::TaskClass::Background, async move { tx2.send(3).await });

		tokio::time::sleep(Duration::from_millis(10)).await;
		assert_eq!(rx.recv().await, Some(1));

		let result = tokio::time::timeout(Duration::from_millis(100), send_task)
			.await
			.expect("send should unblock after pop")
			.unwrap();
		assert!(result.is_ok());

		tx.close().await;
		assert_eq!(rx.recv().await, Some(2));
		assert_eq!(rx.recv().await, Some(3));
		assert_eq!(rx.recv().await, None);
	}

	// ── CoalesceByKey golden tests ──

	#[derive(Clone, Debug, PartialEq, Eq)]
	struct Msg {
		key: u64,
		value: u64,
	}

	#[tokio::test]
	async fn coalesce_replaces_in_place_preserving_order() {
		let mailbox = Mailbox::coalesce_by_key(4, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(Msg { key: 1, value: 10 }).await;
		let _ = tx.send(Msg { key: 2, value: 20 }).await;
		let _ = tx.send(Msg { key: 3, value: 30 }).await;
		assert!(tx.send(Msg { key: 1, value: 99 }).await.is_ok());

		tx.close().await;
		assert_eq!(rx.recv().await, Some(Msg { key: 1, value: 99 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 2, value: 20 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 3, value: 30 }));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn coalesce_evicts_oldest_when_full_and_no_match() {
		let mailbox = Mailbox::coalesce_by_key(3, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(Msg { key: 1, value: 10 }).await;
		let _ = tx.send(Msg { key: 2, value: 20 }).await;
		let _ = tx.send(Msg { key: 3, value: 30 }).await;
		assert!(tx.send(Msg { key: 4, value: 40 }).await.is_ok());

		tx.close().await;
		assert_eq!(rx.recv().await, Some(Msg { key: 2, value: 20 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 3, value: 30 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 4, value: 40 }));
		assert_eq!(rx.recv().await, None);
	}

	#[tokio::test]
	async fn coalesce_replaces_even_when_full() {
		let mailbox = Mailbox::coalesce_by_key(3, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(Msg { key: 1, value: 10 }).await;
		let _ = tx.send(Msg { key: 2, value: 20 }).await;
		let _ = tx.send(Msg { key: 3, value: 30 }).await;
		assert!(tx.send(Msg { key: 2, value: 99 }).await.is_ok());

		tx.close().await;
		assert_eq!(rx.recv().await, Some(Msg { key: 1, value: 10 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 2, value: 99 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 3, value: 30 }));
		assert_eq!(rx.recv().await, None);
	}

	// ── Closed mailbox tests ──

	#[tokio::test]
	async fn send_on_closed_mailbox_returns_closed() {
		let mailbox = Mailbox::backpressure(4);
		let tx = mailbox.sender();
		tx.close().await;

		assert_eq!(tx.send(1u32).await, Err(MailboxSendError::Closed));
	}

	#[tokio::test]
	async fn recv_drains_then_returns_none_on_close() {
		let mailbox = Mailbox::backpressure(4);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(10u32).await;
		let _ = tx.send(20).await;
		tx.close().await;

		assert_eq!(rx.recv().await, Some(10));
		assert_eq!(rx.recv().await, Some(20));
		assert_eq!(rx.recv().await, None);
		assert_eq!(rx.recv().await, None);
	}

	// ── Interleaved push/pop tests ──

	#[tokio::test]
	async fn coalesce_interleaved_push_pop_preserves_order() {
		let mailbox = Mailbox::coalesce_by_key(3, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(Msg { key: 1, value: 10 }).await;
		let _ = tx.send(Msg { key: 2, value: 20 }).await;
		assert_eq!(rx.recv().await, Some(Msg { key: 1, value: 10 }));

		assert!(tx.send(Msg { key: 2, value: 99 }).await.is_ok());
		let _ = tx.send(Msg { key: 3, value: 30 }).await;

		tx.close().await;
		assert_eq!(rx.recv().await, Some(Msg { key: 2, value: 99 }));
		assert_eq!(rx.recv().await, Some(Msg { key: 3, value: 30 }));
		assert_eq!(rx.recv().await, None);
	}

	// ── Stress test (deterministic xorshift) ──

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

		fn push(&mut self, key: u64, value: u32) {
			if let Some(existing) = self.queue.iter_mut().find(|(k, _)| *k == key) {
				existing.1 = value;
				return;
			}
			if self.queue.len() >= self.capacity {
				let _ = self.queue.pop_front();
			}
			self.queue.push_back((key, value));
		}

		fn pop(&mut self) -> Option<(u64, u32)> {
			self.queue.pop_front()
		}

		fn contents(&self) -> Vec<(u64, u32)> {
			self.queue.iter().copied().collect()
		}
	}

	async fn drain_all<T>(rx: &MailboxReceiver<T>) -> Vec<T> {
		let mut items = Vec::new();
		while let Some(item) = rx.recv().await {
			items.push(item);
		}
		items
	}

	#[tokio::test]
	async fn stress_coalesce_by_key_matches_model() {
		const OPS: usize = 10_000;
		let capacity = 4;
		let mailbox = Mailbox::coalesce_by_key(capacity, |m: &Msg| m.key);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();
		let mut model = KeyedQueueModel::new(capacity);
		let mut rng = Xorshift64::new(0xFEED_FACE);
		let key_space = 6u64;

		for i in 0..OPS {
			if rng.next_usize(10) < 6 {
				let key = rng.next() % key_space;
				let val = i as u32;
				tx.send(Msg { key, value: val as u64 }).await.expect("coalesce send should not fail");
				model.push(key, val);
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
		let mailbox = Mailbox::backpressure(4);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let recv_timeout = tokio::time::timeout(Duration::from_millis(20), rx.recv()).await;
		assert!(recv_timeout.is_err(), "recv on empty should block");

		let _ = tx.send(42u32).await;
		assert_eq!(rx.recv().await, Some(42));
	}

#[tokio::test]
	async fn close_now_is_best_effort() {
		let mailbox = Mailbox::backpressure(4);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let _ = tx.send(1u32).await;
		tx.close_now();

		assert_eq!(rx.recv().await, Some(1));
		assert_eq!(rx.recv().await, None);
	}

	// ── Backpressure concurrency: no silent drops ──

	#[tokio::test]
	async fn backpressure_multi_sender_never_drops() {
		const SENDERS: usize = 8;
		const ITEMS_PER_SENDER: usize = 200;
		let total = SENDERS * ITEMS_PER_SENDER;

		let mailbox = Mailbox::backpressure(2);
		let tx = mailbox.sender();
		let rx = mailbox.receiver();

		let barrier = Arc::new(tokio::sync::Barrier::new(SENDERS));

		let mut handles = Vec::new();
		for sender_id in 0..SENDERS {
			let tx = tx.clone();
			let barrier = Arc::clone(&barrier);
			handles.push(crate::spawn::spawn(crate::TaskClass::Background, async move {
				barrier.wait().await;
				for seq in 0..ITEMS_PER_SENDER {
					let val = (sender_id * ITEMS_PER_SENDER + seq) as u32;
					tx.send(val).await.expect("backpressure send must not fail");
				}
			}));
		}

		let receiver = crate::spawn::spawn(crate::TaskClass::Background, async move {
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

		let mut sorted = received;
		sorted.sort();
		let expected: Vec<u32> = (0..total as u32).collect();
		assert_eq!(sorted, expected, "all items delivered without loss or duplication");
	}

	#[tokio::test]
	async fn backpressure_send_returns_closed_when_closed_while_waiting() {
		let mailbox = Mailbox::backpressure(1);
		let tx = mailbox.sender();
		let _rx = mailbox.receiver();

		let _ = tx.send(1u32).await;

		let tx2 = tx.clone();
		let send_task = crate::spawn::spawn(crate::TaskClass::Background, async move { tx2.send(2).await });

		tokio::time::sleep(Duration::from_millis(10)).await;
		tx.close().await;

		let result = tokio::time::timeout(Duration::from_millis(100), send_task)
			.await
			.expect("blocked send should wake on close")
			.unwrap();
		assert_eq!(result, Err(MailboxSendError::Closed));
	}
}
