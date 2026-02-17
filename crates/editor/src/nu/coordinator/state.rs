use std::collections::{HashMap, VecDeque};

use tokio::task::JoinHandle;
use xeno_nu_api::ExportId;

use crate::msg::MsgSender;
use crate::nu::ctx::NuCtxEvent;
use crate::nu::executor::NuExecutor;
use crate::nu::{CachedHookId, NuRuntime};
use crate::types::Invocation;

/// Stable identity for a Nu evaluation request.
///
/// `runtime_epoch` is incremented on every runtime swap. `seq` is a per-epoch
/// monotonic counter used to disambiguate concurrent jobs in the same epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NuEvalToken {
	pub runtime_epoch: u64,
	pub seq: u64,
}

/// Hook pipeline phase used for debug visibility and invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookPipelinePhase {
	Idle,
	HookQueued,
	HookInFlight,
}

impl HookPipelinePhase {
	pub(crate) const fn label(self) -> &'static str {
		match self {
			Self::Idle => "idle",
			Self::HookQueued => "queued",
			Self::HookInFlight => "in_flight",
		}
	}
}

/// A queued Nu post-hook awaiting evaluation.
#[derive(Debug, Clone)]
pub(crate) struct QueuedNuHook {
	pub event: NuCtxEvent,
}

/// Tracks a single in-flight Nu hook evaluation.
#[derive(Debug)]
pub(crate) struct InFlightNuHook {
	pub token: NuEvalToken,
}

/// A scheduled macro awaiting its delay timer.
pub(crate) struct ScheduledEntry {
	pub handle: JoinHandle<()>,
	pub token: u64,
}

/// Message payload for a fired scheduled macro.
#[derive(Debug, Clone)]
pub struct NuScheduleFiredMsg {
	pub key: String,
	pub token: u64,
	pub name: String,
	pub args: Vec<String>,
}

/// Unified Nu pipeline state for runtime, executor, and hook/macro lifecycle.
pub(crate) struct NuCoordinatorState {
	runtime: Option<NuRuntime>,
	executor: Option<NuExecutor>,
	hook_id: CachedHookId,
	hook_depth: u8,
	macro_depth: u8,
	hook_queue: VecDeque<QueuedNuHook>,
	hook_in_flight: Option<InFlightNuHook>,
	runtime_epoch: u64,
	hook_eval_seq_next: u64,
	stop_scope_generation: u64,
	hook_dropped_total: u64,
	scheduled: HashMap<String, ScheduledEntry>,
	scheduled_seq: u64,
	macro_decl_cache: HashMap<String, Option<ExportId>>,
}

impl NuCoordinatorState {
	pub(crate) fn new() -> Self {
		Self {
			runtime: None,
			executor: None,
			hook_id: CachedHookId::default(),
			hook_depth: 0,
			macro_depth: 0,
			hook_queue: VecDeque::new(),
			hook_in_flight: None,
			runtime_epoch: 0,
			hook_eval_seq_next: 0,
			stop_scope_generation: 0,
			hook_dropped_total: 0,
			scheduled: HashMap::new(),
			scheduled_seq: 0,
			macro_decl_cache: HashMap::new(),
		}
	}

	pub(crate) fn set_runtime(&mut self, runtime: Option<NuRuntime>) {
		self.executor = None;
		self.hook_queue.clear();
		self.hook_in_flight = None;
		self.macro_decl_cache.clear();
		for (_, entry) in self.scheduled.drain() {
			entry.handle.abort();
		}
		self.runtime_epoch = self.runtime_epoch.wrapping_add(1);
		self.hook_eval_seq_next = 0;
		self.runtime = runtime;
		self.hook_id = self
			.runtime
			.as_ref()
			.map(|rt| CachedHookId {
				on_hook: rt.find_script_decl("on_hook"),
			})
			.unwrap_or_default();
		self.executor = self.runtime.as_ref().map(|rt| NuExecutor::new(rt.clone()));
	}

	pub(crate) fn runtime(&self) -> Option<&NuRuntime> {
		self.runtime.as_ref()
	}

	pub(crate) fn ensure_executor(&mut self) -> Option<&NuExecutor> {
		if self.executor.is_none()
			&& let Some(runtime) = self.runtime.clone()
		{
			self.executor = Some(NuExecutor::new(runtime));
		}
		self.executor.as_ref()
	}

	pub(crate) fn executor(&self) -> Option<&NuExecutor> {
		self.executor.as_ref()
	}

	pub(crate) fn executor_client(&self) -> Option<NuExecutor> {
		self.executor.as_ref().map(NuExecutor::client)
	}

	/// Resolve a macro function name to its declaration ID, with positive
	/// and negative caching. Cache is cleared on runtime swap.
	pub(crate) fn resolve_macro_decl_cached(&mut self, name: &str) -> Option<ExportId> {
		if let Some(hit) = self.macro_decl_cache.get(name) {
			return *hit;
		}
		let resolved = self.runtime.as_ref().and_then(|rt| rt.find_script_decl(name));
		self.macro_decl_cache.insert(name.to_string(), resolved);
		resolved
	}

	#[cfg(test)]
	pub(crate) fn macro_cache_len(&self) -> usize {
		self.macro_decl_cache.len()
	}

	pub(crate) fn on_hook_decl(&self) -> Option<ExportId> {
		self.hook_id.on_hook
	}

	pub(crate) fn has_on_hook_decl(&self) -> bool {
		self.hook_id.on_hook.is_some()
	}

	#[cfg(test)]
	pub(crate) fn hook_id(&self) -> &CachedHookId {
		&self.hook_id
	}

	pub(crate) fn macro_depth(&self) -> u8 {
		self.macro_depth
	}

	pub(crate) fn inc_macro_depth(&mut self) {
		self.macro_depth = self.macro_depth.saturating_add(1);
	}

	pub(crate) fn dec_macro_depth(&mut self) {
		self.macro_depth = self.macro_depth.saturating_sub(1);
	}

	pub(crate) fn in_hook_drain(&self) -> bool {
		self.hook_depth > 0
	}

	pub(crate) fn inc_hook_depth(&mut self) {
		self.hook_depth = self.hook_depth.saturating_add(1);
	}

	pub(crate) fn dec_hook_depth(&mut self) {
		self.hook_depth = self.hook_depth.saturating_sub(1);
	}

	pub(crate) fn enqueue_hook(&mut self, event: NuCtxEvent, max_pending: usize) -> bool {
		// Coalesce only consecutive same-kind events; preserve interleaved ordering.
		if let Some(back) = self.hook_queue.back_mut()
			&& back.event.same_kind(&event)
		{
			back.event = event;
			return false;
		}

		// New kind — append. Drop oldest if over cap (should rarely happen).
		let mut dropped = false;
		if self.hook_queue.len() >= max_pending {
			self.hook_queue.pop_front();
			self.hook_dropped_total += 1;
			dropped = true;
		}

		self.hook_queue.push_back(QueuedNuHook { event });
		dropped
	}

	pub(crate) fn pop_queued_hook(&mut self) -> Option<QueuedNuHook> {
		self.hook_queue.pop_front()
	}

	pub(crate) fn has_queued_hooks(&self) -> bool {
		!self.hook_queue.is_empty()
	}

	pub(crate) fn hook_queue_len(&self) -> usize {
		self.hook_queue.len()
	}

	#[cfg(test)]
	pub(crate) fn set_hook_in_flight(&mut self, in_flight: InFlightNuHook) {
		self.hook_in_flight = Some(in_flight);
	}

	/// Mark a queued hook as in-flight and return the event for evaluation.
	pub(crate) fn begin_hook_eval(&mut self, token: NuEvalToken, queued: QueuedNuHook) -> NuCtxEvent {
		let event_for_eval = queued.event;
		self.hook_in_flight = Some(InFlightNuHook { token });
		event_for_eval
	}

	pub(crate) fn hook_in_flight(&self) -> Option<&InFlightNuHook> {
		self.hook_in_flight.as_ref()
	}

	pub(crate) fn take_hook_in_flight(&mut self) -> Option<InFlightNuHook> {
		self.hook_in_flight.take()
	}

	pub(crate) fn hook_in_flight_token(&self) -> Option<NuEvalToken> {
		self.hook_in_flight.as_ref().map(|i| i.token)
	}

	/// Complete a hook eval and clear in-flight state when token matches.
	///
	/// Returns `false` for stale completions from older runtime epochs.
	pub(crate) fn complete_hook_eval(&mut self, token: NuEvalToken) -> bool {
		if self.hook_in_flight_token() != Some(token) {
			return false;
		}
		let _ = self.take_hook_in_flight();
		true
	}

	pub(crate) fn next_hook_eval_token(&mut self) -> NuEvalToken {
		let token = NuEvalToken {
			runtime_epoch: self.runtime_epoch,
			seq: self.hook_eval_seq_next,
		};
		self.hook_eval_seq_next = self.hook_eval_seq_next.wrapping_add(1);
		token
	}

	pub(crate) fn hook_eval_seq_next(&self) -> u64 {
		self.hook_eval_seq_next
	}

	pub(crate) fn runtime_epoch(&self) -> u64 {
		self.runtime_epoch
	}

	pub(crate) fn hook_phase(&self) -> HookPipelinePhase {
		if self.hook_depth > 0 || self.hook_in_flight.is_some() {
			HookPipelinePhase::HookInFlight
		} else if !self.hook_queue.is_empty() {
			HookPipelinePhase::HookQueued
		} else {
			HookPipelinePhase::Idle
		}
	}

	/// Returns the current Nu stop-propagation scope generation.
	pub(crate) fn current_stop_scope_generation(&self) -> u64 {
		self.stop_scope_generation
	}

	/// Advances stop-propagation scope generation and returns the prior value.
	pub(crate) fn advance_stop_scope_generation(&mut self) -> u64 {
		let previous = self.stop_scope_generation;
		self.stop_scope_generation = self.stop_scope_generation.wrapping_add(1);
		previous
	}

	/// Clear all pending hook work for stop-propagation semantics.
	pub(crate) fn clear_hook_work_on_stop_propagation(&mut self) {
		self.hook_queue.clear();
	}

	pub(crate) fn hook_dropped_total(&self) -> u64 {
		self.hook_dropped_total
	}

	/// Schedule a macro to fire after `delay_ms`, replacing any previous schedule with the same key.
	pub(crate) fn schedule_macro(&mut self, key: String, delay_ms: u64, name: String, args: Vec<String>, msg_tx: &MsgSender) {
		if let Some(existing) = self.scheduled.remove(&key) {
			existing.handle.abort();
		}
		self.scheduled_seq = self.scheduled_seq.wrapping_add(1);
		let token = self.scheduled_seq;
		let tx = msg_tx.clone();
		let fire_key = key.clone();
		let handle = xeno_worker::spawn(xeno_worker::TaskClass::Background, async move {
			tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
			let _ = tx.send(crate::msg::EditorMsg::NuScheduleFired(NuScheduleFiredMsg {
				key: fire_key,
				token,
				name,
				args,
			}));
		});
		self.scheduled.insert(key, ScheduledEntry { handle, token });
	}

	/// Cancel a scheduled macro by key. No-op if key doesn't exist.
	pub(crate) fn cancel_schedule(&mut self, key: &str) {
		if let Some(entry) = self.scheduled.remove(key) {
			entry.handle.abort();
		}
	}

	/// Handle a fired scheduled macro. Verifies token, enqueues invocation
	/// into the runtime work queue.
	pub(crate) fn apply_schedule_fired(&mut self, msg: NuScheduleFiredMsg) -> Option<Invocation> {
		let Some(entry) = self.scheduled.get(&msg.key) else {
			return None;
		};
		if entry.token != msg.token {
			return None;
		}
		let _ = self.scheduled.remove(&msg.key);
		Some(Invocation::Nu {
			name: msg.name,
			args: msg.args,
		})
	}
}

impl Default for NuCoordinatorState {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::nu::NuRuntime;

	fn make_runtime(script: &str) -> NuRuntime {
		let temp = tempfile::tempdir().expect("temp dir should exist");
		std::fs::write(temp.path().join("xeno.nu"), script).expect("write should succeed");
		let path = temp.path().to_path_buf();
		NuRuntime::load(&path).expect("runtime should load")
	}

	#[test]
	fn macro_decl_cache_hits() {
		let runtime = make_runtime("export def go [] { xeno effect dispatch editor stats }");
		let mut state = NuCoordinatorState::new();
		state.set_runtime(Some(runtime));

		let first = state.resolve_macro_decl_cached("go");
		assert!(first.is_some());
		let second = state.resolve_macro_decl_cached("go");
		assert_eq!(first, second);
		assert_eq!(state.macro_cache_len(), 1);
	}

	#[test]
	fn macro_decl_cache_negative() {
		let runtime = make_runtime("export def go [] { null }");
		let mut state = NuCoordinatorState::new();
		state.set_runtime(Some(runtime));

		assert!(state.resolve_macro_decl_cached("missing").is_none());
		assert!(state.resolve_macro_decl_cached("missing").is_none());
		assert_eq!(state.macro_cache_len(), 1);
	}

	#[test]
	fn set_runtime_clears_macro_cache() {
		let runtime = make_runtime("export def go [] { null }");
		let mut state = NuCoordinatorState::new();
		state.set_runtime(Some(runtime));
		let _ = state.resolve_macro_decl_cached("go");
		assert_eq!(state.macro_cache_len(), 1);

		state.set_runtime(None);
		assert_eq!(state.macro_cache_len(), 0);
	}

	#[test]
	fn enqueue_replaces_consecutive_same_kind() {
		let mut state = NuCoordinatorState::new();
		let a1 = NuCtxEvent::ActionPost {
			name: "a1".into(),
			result: "ok".into(),
		};
		let a2 = NuCtxEvent::ActionPost {
			name: "a2".into(),
			result: "ok".into(),
		};
		let b1 = NuCtxEvent::ModeChange {
			from: "Normal".into(),
			to: "Insert".into(),
		};

		state.enqueue_hook(a1, 64);
		state.enqueue_hook(a2, 64);
		state.enqueue_hook(b1, 64);

		// Queue should be [A2, B1] — consecutive A was coalesced.
		assert_eq!(state.hook_queue_len(), 2);
		let first = state.pop_queued_hook().unwrap();
		assert!(matches!(first.event, NuCtxEvent::ActionPost { ref name, .. } if name == "a2"));
		let second = state.pop_queued_hook().unwrap();
		assert!(matches!(second.event, NuCtxEvent::ModeChange { .. }));
	}

	#[test]
	fn enqueue_interleaved_same_kind_preserves_both_events() {
		let mut state = NuCoordinatorState::new();
		state.enqueue_hook(
			NuCtxEvent::ActionPost {
				name: "a1".into(),
				result: "ok".into(),
			},
			64,
		);
		state.enqueue_hook(
			NuCtxEvent::ModeChange {
				from: "Normal".into(),
				to: "Insert".into(),
			},
			64,
		);
		state.enqueue_hook(
			NuCtxEvent::ActionPost {
				name: "a2".into(),
				result: "ok".into(),
			},
			64,
		);

		assert_eq!(state.hook_queue_len(), 3);
		let first = state.pop_queued_hook().unwrap();
		assert!(matches!(first.event, NuCtxEvent::ActionPost { ref name, .. } if name == "a1"));
		let second = state.pop_queued_hook().unwrap();
		assert!(matches!(second.event, NuCtxEvent::ModeChange { .. }));
		let third = state.pop_queued_hook().unwrap();
		assert!(matches!(third.event, NuCtxEvent::ActionPost { ref name, .. } if name == "a2"));
	}
}
