//! Cooperative execution gate for interactive and background work.
//!
//! Provides lightweight guards that enforce ordering between interactive tasks
//! and background execution phases.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::Notify;

/// Gate for enforcing strict ordering between interactive and background tasks.
#[derive(Debug, Clone)]
pub struct ExecutionGate {
	/// Count of interactive tasks currently in flight.
	interactive_in_flight: Arc<AtomicUsize>,
	/// Notification for state changes (interactive completion or gate open).
	state_notify: Arc<Notify>,
	/// Depth of nested background open scopes.
	background_open_depth: Arc<AtomicUsize>,
}

impl Default for ExecutionGate {
	fn default() -> Self {
		Self {
			interactive_in_flight: Arc::new(AtomicUsize::new(0)),
			state_notify: Arc::new(Notify::new()),
			background_open_depth: Arc::new(AtomicUsize::new(0)),
		}
	}
}

impl ExecutionGate {
	pub fn new() -> Self {
		Self::default()
	}

	/// Enters an interactive task, returning a guard that must be held until completion.
	///
	/// The guard ensures the in-flight counter is decremented even if the task is aborted.
	pub fn enter_interactive(&self) -> InteractiveGuard {
		self.interactive_in_flight.fetch_add(1, Ordering::SeqCst);
		InteractiveGuard {
			active: self.interactive_in_flight.clone(),
			notify: self.state_notify.clone(),
		}
	}

	/// Waits until background tasks are allowed to proceed.
	pub async fn wait_for_background(&self) {
		loop {
			// Register interest before checking condition to avoid race
			let notified = self.state_notify.notified();

			if self.background_open_depth.load(Ordering::SeqCst) > 0 || self.interactive_in_flight.load(Ordering::SeqCst) == 0 {
				return;
			}

			notified.await;
		}
	}

	/// Explicitly opens the gate for background tasks (e.g. during drain).
	///
	/// Returns a guard that decrements the scope depth when dropped.
	pub fn open_background_scope(&self) -> BackgroundOpenGuard {
		self.background_open_depth.fetch_add(1, Ordering::SeqCst);
		self.state_notify.notify_waiters();
		BackgroundOpenGuard {
			depth: self.background_open_depth.clone(),
		}
	}

	/// Returns true if there are any interactive tasks in flight.
	pub fn is_interactive_active(&self) -> bool {
		self.interactive_in_flight.load(Ordering::SeqCst) > 0
	}
}

/// Guard tracking an in-flight interactive task.
pub struct InteractiveGuard {
	active: Arc<AtomicUsize>,
	notify: Arc<Notify>,
}

impl Drop for InteractiveGuard {
	fn drop(&mut self) {
		let prev = self.active.fetch_sub(1, Ordering::SeqCst);
		debug_assert!(prev > 0, "interactive_in_flight underflow");
		self.notify.notify_waiters();
	}
}

/// Guard that keeps the background gate open.
pub struct BackgroundOpenGuard {
	depth: Arc<AtomicUsize>,
}

impl Drop for BackgroundOpenGuard {
	fn drop(&mut self) {
		let prev = self.depth.fetch_sub(1, Ordering::SeqCst);
		debug_assert!(prev > 0, "background_open_depth underflow");
	}
}

#[cfg(test)]
mod tests;
