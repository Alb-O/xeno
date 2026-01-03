//! Hook emission functions for triggering hooks on events.

use tracing::warn;

use super::HOOKS;
use super::context::{HookContext, MutableHookContext};
use super::types::{BoxFuture, HookAction, HookHandler, HookMutability, HookResult};

/// Emit an event to all registered hooks.
///
/// Hooks are executed in priority order (lower priority runs first).
/// Sync hooks complete immediately; async hooks are awaited in sequence.
///
/// Returns [`HookResult::Cancel`] if any hook cancels, otherwise [`HookResult::Continue`].
pub async fn emit(ctx: &HookContext<'_>) -> HookResult {
	let event = ctx.event();
	let mut matching: Vec<_> = HOOKS.iter().filter(|h| h.event == event).collect();
	matching.sort_by_key(|h| h.priority);

	for hook in matching {
		if hook.mutability != HookMutability::Immutable {
			continue;
		}
		let handler = match hook.handler {
			HookHandler::Immutable(handler) => handler,
			HookHandler::Mutable(_) => continue,
		};
		let result = match handler(ctx) {
			HookAction::Done(result) => result,
			HookAction::Async(fut) => fut.await,
		};
		if result == HookResult::Cancel {
			return HookResult::Cancel;
		}
	}
	HookResult::Continue
}

/// Emit an event synchronously, ignoring any async hooks.
///
/// This is useful in contexts where async is not available. Async hooks
/// will log a warning and be skipped.
pub fn emit_sync(ctx: &HookContext<'_>) -> HookResult {
	let event = ctx.event();
	let mut matching: Vec<_> = HOOKS.iter().filter(|h| h.event == event).collect();
	matching.sort_by_key(|h| h.priority);

	for hook in matching {
		if hook.mutability != HookMutability::Immutable {
			continue;
		}
		let handler = match hook.handler {
			HookHandler::Immutable(handler) => handler,
			HookHandler::Mutable(_) => continue,
		};
		match handler(ctx) {
			HookAction::Done(result) => {
				if result == HookResult::Cancel {
					return HookResult::Cancel;
				}
			}
			HookAction::Async(_) => {
				warn!(
					hook = hook.name,
					"Hook returned async action but emit_sync was called; skipping"
				);
			}
		}
	}
	HookResult::Continue
}

/// Emit a mutable event to all registered mutable hooks.
///
/// Returns [`HookResult::Cancel`] if any hook cancels, otherwise [`HookResult::Continue`].
pub async fn emit_mutable(ctx: &mut MutableHookContext<'_>) -> HookResult {
	let event = ctx.event;
	let mut matching: Vec<_> = HOOKS.iter().filter(|h| h.event == event).collect();
	matching.sort_by_key(|h| h.priority);

	for hook in matching {
		if hook.mutability != HookMutability::Mutable {
			continue;
		}
		let handler = match hook.handler {
			HookHandler::Mutable(handler) => handler,
			HookHandler::Immutable(_) => continue,
		};
		let result = match handler(ctx) {
			HookAction::Done(result) => result,
			HookAction::Async(fut) => fut.await,
		};
		if result == HookResult::Cancel {
			return HookResult::Cancel;
		}
	}
	HookResult::Continue
}

/// Trait for scheduling async hook futures.
///
/// This allows sync emission to queue async hooks without coupling `xeno-registry-hooks`
/// to any specific runtime. The caller provides an implementor that stores futures
/// for later execution.
pub trait HookScheduler {
	/// Queue an async hook future for later execution.
	fn schedule(&mut self, fut: BoxFuture);
}

/// Emit an event synchronously, scheduling async hooks for later execution.
///
/// Sync hooks run immediately and can cancel the operation. Async hooks are
/// queued via the provided scheduler and will run later (they cannot cancel
/// since the operation has already proceeded).
///
/// Returns [`HookResult::Cancel`] if any sync hook cancels, otherwise [`HookResult::Continue`].
pub fn emit_sync_with<S: HookScheduler>(ctx: &HookContext<'_>, scheduler: &mut S) -> HookResult {
	let event = ctx.event();
	let mut matching: Vec<_> = HOOKS.iter().filter(|h| h.event == event).collect();
	matching.sort_by_key(|h| h.priority);

	for hook in matching {
		if hook.mutability != HookMutability::Immutable {
			continue;
		}
		let handler = match hook.handler {
			HookHandler::Immutable(handler) => handler,
			HookHandler::Mutable(_) => continue,
		};
		match handler(ctx) {
			HookAction::Done(result) => {
				if result == HookResult::Cancel {
					return HookResult::Cancel;
				}
			}
			HookAction::Async(fut) => {
				scheduler.schedule(fut);
			}
		}
	}
	HookResult::Continue
}
