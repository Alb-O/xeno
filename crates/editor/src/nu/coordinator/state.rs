use std::collections::VecDeque;

use xeno_nu_runtime::FunctionId;

use crate::nu::executor::NuExecutor;
use crate::nu::{CachedHookIds, NuHook, NuRuntime};
use crate::types::Invocation;

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
	pub job_id: u64,
	pub hook: NuHook,
	pub args: Vec<String>,
	pub retries: u8,
}

/// Unified Nu pipeline state for runtime, executor, and hook/macro lifecycle.
pub(crate) struct NuCoordinatorState {
	runtime: Option<NuRuntime>,
	executor: Option<NuExecutor>,
	hook_ids: CachedHookIds,
	hook_depth: u8,
	macro_depth: u8,
	hook_queue: VecDeque<QueuedNuHook>,
	hook_in_flight: Option<InFlightNuHook>,
	hook_job_next: u64,
	hook_pending_invocations: VecDeque<Invocation>,
	hook_dropped_total: u64,
	hook_failed_total: u64,
}

impl NuCoordinatorState {
	pub(crate) fn new() -> Self {
		Self {
			runtime: None,
			executor: None,
			hook_ids: CachedHookIds::default(),
			hook_depth: 0,
			macro_depth: 0,
			hook_queue: VecDeque::new(),
			hook_in_flight: None,
			hook_job_next: 0,
			hook_pending_invocations: VecDeque::new(),
			hook_dropped_total: 0,
			hook_failed_total: 0,
		}
	}

	pub(crate) fn set_runtime(&mut self, runtime: Option<NuRuntime>) {
		self.executor = None;
		self.hook_queue.clear();
		self.hook_pending_invocations.clear();
		self.hook_in_flight = None;
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

	pub(crate) fn hook_decl(&self, hook: NuHook) -> Option<FunctionId> {
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
	}

	pub(crate) fn dec_hook_depth(&mut self) {
		self.hook_depth = self.hook_depth.saturating_sub(1);
	}

	pub(crate) fn enqueue_hook(&mut self, hook: NuHook, args: Vec<String>, max_pending: usize) -> bool {
		if let Some(back) = self.hook_queue.back_mut()
			&& back.hook == hook
		{
			back.args = args;
			back.retries = 0;
			return false;
		}

		let mut dropped = false;
		if self.hook_queue.len() >= max_pending {
			self.hook_queue.pop_front();
			self.hook_dropped_total += 1;
			dropped = true;
		}

		self.hook_queue.push_back(QueuedNuHook { hook, args, retries: 0 });
		dropped
	}

	pub(crate) fn pop_queued_hook(&mut self) -> Option<QueuedNuHook> {
		self.hook_queue.pop_front()
	}

	pub(crate) fn push_front_queued_hook(&mut self, hook: QueuedNuHook) {
		self.hook_queue.push_front(hook);
	}

	pub(crate) fn has_queued_hooks(&self) -> bool {
		!self.hook_queue.is_empty()
	}

	pub(crate) fn hook_queue_len(&self) -> usize {
		self.hook_queue.len()
	}

	pub(crate) fn set_hook_in_flight(&mut self, in_flight: InFlightNuHook) {
		self.hook_in_flight = Some(in_flight);
	}

	pub(crate) fn hook_in_flight(&self) -> Option<&InFlightNuHook> {
		self.hook_in_flight.as_ref()
	}

	pub(crate) fn take_hook_in_flight(&mut self) -> Option<InFlightNuHook> {
		self.hook_in_flight.take()
	}

	pub(crate) fn hook_in_flight_job_id(&self) -> Option<u64> {
		self.hook_in_flight.as_ref().map(|i| i.job_id)
	}

	pub(crate) fn next_hook_job_id(&mut self) -> u64 {
		let job_id = self.hook_job_next;
		self.hook_job_next = self.hook_job_next.wrapping_add(1);
		job_id
	}

	pub(crate) fn hook_job_next(&self) -> u64 {
		self.hook_job_next
	}

	pub(crate) fn extend_pending_hook_invocations(&mut self, invocations: Vec<Invocation>) {
		self.hook_pending_invocations.extend(invocations);
	}

	pub(crate) fn pop_pending_hook_invocation(&mut self) -> Option<Invocation> {
		self.hook_pending_invocations.pop_front()
	}

	pub(crate) fn has_pending_hook_invocations(&self) -> bool {
		!self.hook_pending_invocations.is_empty()
	}

	pub(crate) fn pending_hook_invocations_len(&self) -> usize {
		self.hook_pending_invocations.len()
	}

	pub(crate) fn hook_dropped_total(&self) -> u64 {
		self.hook_dropped_total
	}

	pub(crate) fn hook_failed_total(&self) -> u64 {
		self.hook_failed_total
	}

	pub(crate) fn inc_hook_failed_total(&mut self) {
		self.hook_failed_total = self.hook_failed_total.saturating_add(1);
	}
}

impl Default for NuCoordinatorState {
	fn default() -> Self {
		Self::new()
	}
}
