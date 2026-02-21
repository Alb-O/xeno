use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio_util::sync::CancellationToken;

/// Monotonic generation clock for supervised worker lifecycles.
#[derive(Debug, Default, Clone)]
pub(crate) struct GenerationClock {
	next: Arc<AtomicU64>,
}

impl GenerationClock {
	/// Creates a new generation clock starting at generation 1.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns the next generation ID.
	pub fn next(&self) -> u64 {
		self.next.fetch_add(1, Ordering::AcqRel).wrapping_add(1)
	}
}

/// Generation-scoped cancellation token for actor/task lifecycles.
#[derive(Debug, Clone)]
pub(crate) struct GenerationToken {
	generation: u64,
	cancel: CancellationToken,
}

impl GenerationToken {
	/// Creates a new generation token.
	pub fn new(generation: u64, cancel: CancellationToken) -> Self {
		Self { generation, cancel }
	}

	/// Returns generation ID.
	pub const fn generation(&self) -> u64 {
		self.generation
	}

	/// Returns true when cancellation is requested.
	pub fn is_cancelled(&self) -> bool {
		self.cancel.is_cancelled()
	}

	/// Requests cancellation.
	pub fn cancel(&self) {
		self.cancel.cancel();
	}

	/// Future resolving when cancellation is requested.
	pub async fn cancelled(&self) {
		self.cancel.cancelled().await;
	}

	/// Creates a child token in the same generation.
	pub fn child(&self) -> Self {
		Self {
			generation: self.generation,
			cancel: self.cancel.child_token(),
		}
	}
}
