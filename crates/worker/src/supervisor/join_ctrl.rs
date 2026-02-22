use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Join coordination state machine for the supervisor task.
///
/// Prevents concurrent shutdown callers from racing: only one caller
/// becomes the "leader" that awaits the join handle, all others wait
/// on the notify until the leader transitions to `Done`.
enum JoinState {
	/// Supervisor task is still owned; first caller to `shutdown` takes it.
	Handle(JoinHandle<()>),
	/// A caller is currently awaiting the join handle.
	Joining,
	/// Supervisor task has completed.
	Done,
}

pub(super) struct SupervisorJoinCtrl {
	state: Mutex<JoinState>,
	done: tokio::sync::Notify,
}

impl SupervisorJoinCtrl {
	pub(super) fn new(handle: JoinHandle<()>) -> Self {
		Self {
			state: Mutex::new(JoinState::Handle(handle)),
			done: tokio::sync::Notify::new(),
		}
	}

	/// Joins the supervisor task, blocking until done. Multiple callers are safe.
	pub(super) async fn join_forever(&self) {
		loop {
			let maybe_handle = {
				let mut st = self.state.lock().await;
				match &*st {
					JoinState::Done => return,
					JoinState::Joining => {
						// Create Notified while lock is held to avoid lost-wakeup race:
						// leader could notify_waiters() between our drop(st) and .await.
						let notified = self.done.notified();
						drop(st);
						notified.await;
						continue;
					}
					JoinState::Handle(_) => {
						// Take the handle — we're the leader.
						let JoinState::Handle(h) = std::mem::replace(&mut *st, JoinState::Joining) else {
							unreachable!()
						};
						Some(h)
					}
				}
			};
			if let Some(h) = maybe_handle {
				let _ = h.await;
				*self.state.lock().await = JoinState::Done;
				self.done.notify_waiters();
				return;
			}
		}
	}

	/// Joins with a deadline. Returns `true` if completed, `false` if timed out.
	pub(super) async fn join_with_timeout(&self, timeout: Duration) -> bool {
		let deadline = tokio::time::Instant::now() + timeout;
		loop {
			let maybe_handle = {
				let mut st = self.state.lock().await;
				match &*st {
					JoinState::Done => return true,
					JoinState::Joining => {
						// Create Notified while lock is held to avoid lost-wakeup race.
						let notified = self.done.notified();
						drop(st);
						tokio::select! {
							_ = notified => continue,
							_ = tokio::time::sleep_until(deadline) => return false,
						}
					}
					JoinState::Handle(_) => {
						let JoinState::Handle(h) = std::mem::replace(&mut *st, JoinState::Joining) else {
							unreachable!()
						};
						Some(h)
					}
				}
			};
			if let Some(mut h) = maybe_handle {
				tokio::select! {
					res = &mut h => {
						let _ = res;
						*self.state.lock().await = JoinState::Done;
						self.done.notify_waiters();
						return true;
					}
					_ = tokio::time::sleep_until(deadline) => {
						// Put the handle back — we couldn't finish in time.
						*self.state.lock().await = JoinState::Handle(h);
						self.done.notify_waiters();
						return false;
					}
				}
			}
		}
	}
}
