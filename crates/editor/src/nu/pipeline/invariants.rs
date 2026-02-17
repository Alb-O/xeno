use crate::nu::NuHook;
use crate::nu::coordinator::{HookEvalFailureTransition, HookPipelinePhase, InFlightNuHook, NuCoordinatorState, QueuedNuHook};
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
/// * Enforced in: `NuCoordinatorState::complete_hook_eval_transport_failure`
/// * Failure symptom: failed hook loops forever and starves the scheduler.
#[cfg_attr(test, test)]
pub(crate) fn test_retry_payload_tracks_single_retry() {
	let mut state = NuCoordinatorState::new();
	let token = state.next_hook_eval_token();
	let args_for_eval = state.begin_hook_eval(
		token,
		QueuedNuHook {
			hook: NuHook::CommandPost,
			args: vec!["write".to_string(), "ok".to_string()],
			retries: 0,
		},
	);
	assert_eq!(args_for_eval, vec!["write".to_string(), "ok".to_string()]);
	assert_eq!(state.complete_hook_eval_transport_failure(token), HookEvalFailureTransition::Retried);

	let retried = state.pop_queued_hook().expect("retry hook should be queued");
	assert_eq!(retried.retries, 1, "first transport failure should schedule single retry");

	let token2 = state.next_hook_eval_token();
	state.begin_hook_eval(token2, retried);
	assert_eq!(
		state.complete_hook_eval_transport_failure(token2),
		HookEvalFailureTransition::RetryExhausted { failed_total: 1 }
	);
	assert_eq!(state.hook_failed_total(), 1, "second transport failure should increment failed total");
}

/// Must drop queued and pending hook work when stop-propagation is requested.
///
/// * Enforced in: `NuCoordinatorState::clear_hook_work_on_stop_propagation`
/// * Failure symptom: stopped hooks still dispatch queued invocations.
#[cfg_attr(test, test)]
pub(crate) fn test_stop_propagation_clears_queued_and_pending() {
	let mut state = NuCoordinatorState::new();
	state.enqueue_hook(NuHook::ActionPost, vec!["a".to_string(), "ok".to_string()], 64);
	state.extend_pending_hook_invocations(vec![Invocation::action("move_right")]);

	state.clear_hook_work_on_stop_propagation();

	assert!(!state.has_queued_hooks(), "stop propagation should clear queued hooks");
	assert!(!state.has_pending_hook_invocations(), "stop propagation should clear pending hook invocations");
	assert_eq!(state.hook_phase(), HookPipelinePhase::Idle);
}

/// Must keep in-flight state when completion token is stale.
///
/// * Enforced in: `NuCoordinatorState::complete_hook_eval`
/// * Failure symptom: stale completions clear active in-flight hook state.
#[cfg_attr(test, test)]
pub(crate) fn test_stale_completion_keeps_inflight_state() {
	let mut state = NuCoordinatorState::new();
	let token = state.next_hook_eval_token();
	state.set_hook_in_flight(InFlightNuHook {
		token,
		hook: NuHook::ActionPost,
		args: vec!["name".to_string(), "ok".to_string()],
		retries: 0,
	});

	let stale = crate::nu::coordinator::NuEvalToken {
		runtime_epoch: token.runtime_epoch.wrapping_add(1),
		seq: token.seq,
	};

	assert!(!state.complete_hook_eval(stale), "stale completion should be ignored");
	assert_eq!(state.hook_in_flight_token(), Some(token), "stale completion must not clear active in-flight");
}

/// Must not clear the active schedule when handling a stale schedule token.
///
/// * Enforced in: `NuCoordinatorState::apply_schedule_fired`
/// * Failure symptom: stale timer messages cancel the latest debounced macro.
#[tokio::test]
pub(crate) async fn test_stale_schedule_token_preserves_active_schedule() {
	use crate::nu::coordinator::NuScheduleFiredMsg;

	let (msg_tx, _msg_rx) = crate::msg::channel();
	let mut state = NuCoordinatorState::new();

	state.schedule_macro("debounce".to_string(), 60_000, "old".to_string(), Vec::new(), &msg_tx); // token 1
	state.schedule_macro("debounce".to_string(), 60_000, "current".to_string(), vec!["arg".to_string()], &msg_tx); // token 2

	let stale = state.apply_schedule_fired(NuScheduleFiredMsg {
		key: "debounce".to_string(),
		token: 1,
		name: "stale".to_string(),
		args: Vec::new(),
	});
	assert!(!stale, "stale token should be ignored");

	let current = state.apply_schedule_fired(NuScheduleFiredMsg {
		key: "debounce".to_string(),
		token: 2,
		name: "current".to_string(),
		args: vec!["arg".to_string()],
	});
	assert!(current, "active token should remain valid after stale message");
	assert!(matches!(
		state.pop_pending_hook_invocation(),
		Some(Invocation::Nu { name, args }) if name == "current" && args == vec!["arg".to_string()]
	));

	// Ensure spawned schedule tasks are cancelled in test teardown.
	state.set_runtime(None);
}
