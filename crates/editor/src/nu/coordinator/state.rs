use std::collections::{HashMap, VecDeque};

use tokio::task::JoinHandle;
use xeno_nu_api::ExportId;

use crate::msg::MsgSender;
use crate::nu::executor::NuExecutor;
use crate::nu::{CachedHookIds, NuHook, NuRuntime};
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
	pub hook: NuHook,
	pub args: Vec<String>,
	pub retries: u8,
}

/// Tracks a single in-flight Nu hook evaluation.
#[derive(Debug)]
pub(crate) struct InFlightNuHook {
	pub token: NuEvalToken,
	pub hook: NuHook,
	pub args: Vec<String>,
	pub retries: u8,
}

/// Result of handling a transport-level hook eval failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookEvalFailureTransition {
	Stale,
	Retried,
	RetryExhausted { failed_total: u64 },
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
	hook_ids: CachedHookIds,
	hook_depth: u8,
	macro_depth: u8,
	hook_phase: HookPipelinePhase,
	hook_queue: VecDeque<QueuedNuHook>,
	hook_in_flight: Option<InFlightNuHook>,
	runtime_epoch: u64,
	hook_eval_seq_next: u64,
	stop_scope_generation: u64,
	hook_dropped_total: u64,
	hook_failed_total: u64,
	scheduled: HashMap<String, ScheduledEntry>,
	scheduled_seq: u64,
}

impl NuCoordinatorState {
	pub(crate) fn new() -> Self {
		Self {
			runtime: None,
			executor: None,
			hook_ids: CachedHookIds::default(),
			hook_depth: 0,
			macro_depth: 0,
			hook_phase: HookPipelinePhase::Idle,
			hook_queue: VecDeque::new(),
			hook_in_flight: None,
			runtime_epoch: 0,
			hook_eval_seq_next: 0,
			stop_scope_generation: 0,
			hook_dropped_total: 0,
			hook_failed_total: 0,
			scheduled: HashMap::new(),
			scheduled_seq: 0,
		}
	}

	pub(crate) fn set_runtime(&mut self, runtime: Option<NuRuntime>) {
		self.executor = None;
		self.hook_queue.clear();
		self.hook_in_flight = None;
		for (_, entry) in self.scheduled.drain() {
			entry.handle.abort();
		}
		self.runtime_epoch = self.runtime_epoch.wrapping_add(1);
		self.hook_eval_seq_next = 0;
		self.runtime = runtime;
		self.hook_ids = self
			.runtime
			.as_ref()
			.map(|rt| CachedHookIds {
				on_action_post: rt.find_script_decl("on_action_post"),
				on_command_post: rt.find_script_decl("on_command_post"),
				on_editor_command_post: rt.find_script_decl("on_editor_command_post"),
				on_mode_change: rt.find_script_decl("on_mode_change"),
				on_buffer_open: rt.find_script_decl("on_buffer_open"),
			})
			.unwrap_or_default();
		self.executor = self.runtime.as_ref().map(|rt| NuExecutor::new(rt.clone()));
		self.refresh_hook_phase();
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

	pub(crate) fn restart_executor(&mut self) {
		if let Some(runtime) = self.runtime.clone() {
			self.executor = Some(NuExecutor::new(runtime));
		}
	}

	pub(crate) fn executor(&self) -> Option<&NuExecutor> {
		self.executor.as_ref()
	}

	pub(crate) fn executor_client(&self) -> Option<NuExecutor> {
		self.executor.as_ref().map(NuExecutor::client)
	}

	pub(crate) fn hook_decl(&self, hook: NuHook) -> Option<ExportId> {
		match hook {
			NuHook::ActionPost => self.hook_ids.on_action_post,
			NuHook::CommandPost => self.hook_ids.on_command_post,
			NuHook::EditorCommandPost => self.hook_ids.on_editor_command_post,
			NuHook::ModeChange => self.hook_ids.on_mode_change,
			NuHook::BufferOpen => self.hook_ids.on_buffer_open,
		}
	}

	pub(crate) fn has_hook_decl(&self, hook: NuHook) -> bool {
		self.hook_decl(hook).is_some()
	}

	#[cfg(test)]
	pub(crate) fn hook_ids(&self) -> &CachedHookIds {
		&self.hook_ids
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
		self.refresh_hook_phase();
	}

	pub(crate) fn dec_hook_depth(&mut self) {
		self.hook_depth = self.hook_depth.saturating_sub(1);
		self.refresh_hook_phase();
	}

	pub(crate) fn enqueue_hook(&mut self, hook: NuHook, args: Vec<String>, max_pending: usize) -> bool {
		if let Some(back) = self.hook_queue.back_mut()
			&& back.hook == hook
		{
			back.args = args;
			back.retries = 0;
			self.refresh_hook_phase();
			return false;
		}

		let mut dropped = false;
		if self.hook_queue.len() >= max_pending {
			self.hook_queue.pop_front();
			self.hook_dropped_total += 1;
			dropped = true;
		}

		self.hook_queue.push_back(QueuedNuHook { hook, args, retries: 0 });
		self.refresh_hook_phase();
		dropped
	}

	pub(crate) fn pop_queued_hook(&mut self) -> Option<QueuedNuHook> {
		let queued = self.hook_queue.pop_front();
		self.refresh_hook_phase();
		queued
	}

	pub(crate) fn push_front_queued_hook(&mut self, hook: QueuedNuHook) {
		self.hook_queue.push_front(hook);
		self.refresh_hook_phase();
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
		self.refresh_hook_phase();
	}

	/// Mark a queued hook as in-flight and return owned args for evaluation.
	///
	/// Keeps args/retry metadata in in-flight state for failure recovery while
	/// returning a cloned args payload for the async executor request.
	pub(crate) fn begin_hook_eval(&mut self, token: NuEvalToken, queued: QueuedNuHook) -> Vec<String> {
		let args_for_eval = queued.args.clone();
		self.hook_in_flight = Some(InFlightNuHook {
			token,
			hook: queued.hook,
			args: queued.args,
			retries: queued.retries,
		});
		self.refresh_hook_phase();
		args_for_eval
	}

	pub(crate) fn hook_in_flight(&self) -> Option<&InFlightNuHook> {
		self.hook_in_flight.as_ref()
	}

	pub(crate) fn take_hook_in_flight(&mut self) -> Option<InFlightNuHook> {
		let in_flight = self.hook_in_flight.take();
		self.refresh_hook_phase();
		in_flight
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

	/// Handle hook eval transport failure with built-in single-retry policy.
	///
	/// Retry is only scheduled for first-failure in-flight requests.
	pub(crate) fn complete_hook_eval_transport_failure(&mut self, token: NuEvalToken) -> HookEvalFailureTransition {
		if self.hook_in_flight_token() != Some(token) {
			return HookEvalFailureTransition::Stale;
		}

		let Some(in_flight) = self.take_hook_in_flight() else {
			return HookEvalFailureTransition::Stale;
		};

		if in_flight.retries == 0 {
			self.push_front_queued_hook(QueuedNuHook {
				hook: in_flight.hook,
				args: in_flight.args,
				retries: 1,
			});
			return HookEvalFailureTransition::Retried;
		}

		self.hook_failed_total = self.hook_failed_total.saturating_add(1);
		HookEvalFailureTransition::RetryExhausted {
			failed_total: self.hook_failed_total,
		}
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
		self.hook_phase
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
		self.refresh_hook_phase();
	}

	pub(crate) fn hook_dropped_total(&self) -> u64 {
		self.hook_dropped_total
	}

	pub(crate) fn hook_failed_total(&self) -> u64 {
		self.hook_failed_total
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
		let handle = tokio::spawn(async move {
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
	/// into the runtime deferred invocation mailbox.
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

	fn refresh_hook_phase(&mut self) {
		self.hook_phase = if self.hook_depth > 0 || self.hook_in_flight.is_some() {
			HookPipelinePhase::HookInFlight
		} else if !self.hook_queue.is_empty() {
			HookPipelinePhase::HookQueued
		} else {
			HookPipelinePhase::Idle
		};
	}
}

impl Default for NuCoordinatorState {
	fn default() -> Self {
		Self::new()
	}
}
