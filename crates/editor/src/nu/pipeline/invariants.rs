use crate::nu::NuHook;
use crate::nu::coordinator::{HookPipelinePhase, InFlightNuHook, NuCoordinatorState};
use crate::types::Invocation;

/// Must invalidate in-flight hook identity when runtime generation changes.
///
/// * Enforced in: `NuCoordinatorState::set_runtime`
/// * Failure symptom: stale async hook completions mutate the active runtime state.
#[cfg_attr(test, test)]
pub(crate) fn test_runtime_swap_invalidates_inflight_token() {
	let mut state = NuCoordinatorState::new();
	let token = state.next_hook_eval_token();
	state.set_hook_in_flight(InFlightNuHook {
		token,
		hook: NuHook::ActionPost,
		args: vec!["name".to_string(), "ok".to_string()],
		retries: 0,
	});

	state.set_runtime(None);

	assert_eq!(state.hook_in_flight_token(), None);
	assert_ne!(state.runtime_epoch(), token.runtime_epoch);
	assert_eq!(state.hook_eval_seq_next(), 0);
}

/// Must reflect queue/in-flight/drain lifecycle in the observable hook phase.
///
/// * Enforced in: `NuCoordinatorState::{enqueue_hook,set_hook_in_flight,extend_pending_hook_invocations,pop_pending_hook_invocation}`
/// * Failure symptom: pipeline debugging and invariants lose ordering signal.
#[cfg_attr(test, test)]
pub(crate) fn test_hook_phase_tracks_pipeline_lifecycle() {
	let mut state = NuCoordinatorState::new();
	assert_eq!(state.hook_phase(), HookPipelinePhase::Idle);

	state.enqueue_hook(NuHook::ActionPost, vec!["a".to_string(), "ok".to_string()], 64);
	assert_eq!(state.hook_phase(), HookPipelinePhase::HookQueued);

	let token = state.next_hook_eval_token();
	state.set_hook_in_flight(InFlightNuHook {
		token,
		hook: NuHook::ActionPost,
		args: vec!["a".to_string(), "ok".to_string()],
		retries: 0,
	});
	assert_eq!(state.hook_phase(), HookPipelinePhase::HookInFlight);

	state.extend_pending_hook_invocations(vec![Invocation::command("write", Vec::<String>::new())]);
	assert_eq!(state.hook_phase(), HookPipelinePhase::DrainingHookInvocations);

	let _ = state.pop_pending_hook_invocation();
	let _ = state.take_hook_in_flight();
	let _ = state.pop_queued_hook();
	assert_eq!(state.hook_phase(), HookPipelinePhase::Idle);
}

/// Must retry at most once for a failed in-flight hook.
///
/// * Enforced in: hook requeue logic in `nu::pipeline::apply_nu_hook_eval_done`
/// * Failure symptom: failed hook loops forever and starves the scheduler.
#[cfg_attr(test, test)]
pub(crate) fn test_retry_payload_tracks_single_retry() {
	let mut state = NuCoordinatorState::new();
	let token = state.next_hook_eval_token();
	state.set_hook_in_flight(InFlightNuHook {
		token,
		hook: NuHook::CommandPost,
		args: vec!["write".to_string(), "ok".to_string()],
		retries: 0,
	});

	let in_flight = state.take_hook_in_flight().expect("in-flight hook should exist");
	state.push_front_queued_hook(crate::nu::coordinator::QueuedNuHook {
		hook: in_flight.hook,
		args: in_flight.args,
		retries: 1,
	});

	let queued = state.pop_queued_hook().expect("retry hook should be queued");
	assert_eq!(queued.retries, 1);
}

/// Must drop queued and pending hook work when stop-propagation is requested.
///
/// * Enforced in: `nu::pipeline::apply_hook_effect_batch`
/// * Failure symptom: stopped hooks still dispatch queued invocations.
#[cfg_attr(test, test)]
pub(crate) fn test_stop_propagation_clears_queued_and_pending() {
	let mut state = NuCoordinatorState::new();
	state.enqueue_hook(NuHook::ActionPost, vec!["a".to_string(), "ok".to_string()], 64);
	state.extend_pending_hook_invocations(vec![Invocation::action("move_right")]);

	state.clear_queued_hooks();
	state.clear_pending_hook_invocations();

	assert!(!state.has_queued_hooks(), "stop propagation should clear queued hooks");
	assert!(!state.has_pending_hook_invocations(), "stop propagation should clear pending hook invocations");
	assert_eq!(state.hook_phase(), HookPipelinePhase::Idle);
}
