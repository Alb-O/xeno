//! Unified invocation dispatch and Nu hook pipeline bridge.
//! Anchor ID: XENO_ANCHOR_INVOCATION_PIPELINE
//!
//! # Purpose
//!
//! * Owns the single invocation entry point for actions, commands, editor commands, and Nu macro calls.
//! * Enforces invocation policy gates (capabilities, readonly), pre/post hook emission, and user-facing error shaping.
//! * Bridges invocation outcomes into deferred Nu hook pipeline scheduling.
//!
//! # Mental model
//!
//! * `Invocation` is input intent (`Action`, `Command`, `EditorCommand`, `Nu`).
//! * `run_invocation` is the canonical dispatcher and must be the first stop for user-triggered execution.
//! * Each branch resolves definitions, performs policy checks, executes handler logic, then flushes effects.
//! * Nu post hooks are queued only for non-quit outcomes, then drained later from runtime `pump()`.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`crate::types::Invocation`] | User/script invocation request | Must be dispatched only through `run_invocation` | key handling, command paths, Nu pipelines |
//! | [`crate::types::InvocationPolicy`] | Enforcement policy toggles | Enforcing mode must block capability/readonly violations | `InvocationPolicy::enforcing` |
//! | [`crate::types::InvocationResult`] | Canonical execution outcome | Must preserve quit/force-quit/error propagation | branch handlers in this module |
//! | [`crate::impls::Editor`] | Runtime owner of invocation execution | Must flush queued effects after each command/action execution branch | `run_*_invocation` methods |
//! | [`crate::nu::NuHook`] | Deferred hook kind | Must enqueue only when execution does not request quit | `run_invocation` |
//!
//! # Invariants
//!
//! * Must gate capability violations through `InvocationPolicy` before handler execution.
//! * Must gate readonly edits when policy enforces readonly and target requires edit capability.
//! * Must enqueue Nu post hooks only for non-quit invocation outcomes.
//! * Must cap Nu macro recursion depth to prevent unbounded self-recursion.
//! * Must flush queued effects after action/command/editor-command execution branches.
//!
//! # Data flow
//!
//! 1. Caller builds an [`crate::types::Invocation`] and calls `run_invocation`.
//! 2. Dispatcher resolves target definition and runs policy gates.
//! 3. Handler executes and returns typed outcome/effects.
//! 4. Effects are flushed and transformed into editor mutations and overlay/layer events.
//! 5. Non-quit outcomes enqueue Nu post hooks, later drained by runtime `pump()`.
//!
//! # Lifecycle
//!
//! * Startup: editor initializes invocation surfaces and registry/plugin definitions.
//! * Active: interactive paths repeatedly call `run_invocation` as keys/commands dispatch.
//! * Hook follow-up: Nu hook queue and async eval run outside the direct invocation call path.
//! * Shutdown: scheduler/runtime drain pending work; no special invocation teardown state is retained.
//!
//! # Concurrency & ordering
//!
//! * Invocation dispatch itself is synchronous on the editor thread.
//! * Hook emission uses work-scheduler ordering guarantees from hook runtime.
//! * Nu hook eval is sequentialized through coordinator in-flight state and token checks.
//! * Recursive Nu macro dispatch is bounded by `MAX_NU_MACRO_DEPTH`.
//!
//! # Failure modes & recovery
//!
//! * Unknown target: return `InvocationResult::NotFound`.
//! * Capability violation: return `CapabilityDenied` or log-only continue based on policy.
//! * Readonly violation: emit readonly notification and return `ReadonlyDenied`.
//! * Nu runtime/executor/decode failure: return `CommandError` and notify user.
//! * Nu recursion overflow: return bounded recursion error string.
//!
//! # Recipes
//!
//! * Add a new invocation variant:
//!   1. Extend [`crate::types::Invocation`].
//!   2. Add a `run_invocation` match arm and policy/flush behavior.
//!   3. Decide post-hook contract (if any) and add tests.
//! * Add a new enforcement rule:
//!   1. Add gate before handler call.
//!   2. Route violations through `handle_capability_violation`-style shaping.
//!   3. Add invariant proof in `invariants.rs`.

mod core;

pub(crate) use core::MAX_NU_HOOKS_PER_PUMP;
#[cfg(test)]
pub(crate) use core::{action_post_args, command_post_args, handle_capability_violation};

#[cfg(test)]
use xeno_registry::actions::EditorContext;

#[cfg(test)]
use crate::impls::Editor;
#[cfg(test)]
use crate::types::{Invocation, InvocationPolicy, InvocationResult};

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
