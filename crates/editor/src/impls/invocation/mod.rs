//! Unified invocation dispatch and Nu hook pipeline bridge.
//! Anchor ID: XENO_ANCHOR_INVOCATION_PIPELINE
//!
//! # Purpose
//!
//! * Owns the single invocation entry point for actions, route-aware commands, and Nu macro calls.
//! * Enforces invocation policy gates (capabilities, readonly), pre/post hook emission, and user-facing error shaping.
//! * Bridges invocation outcomes into deferred Nu hook pipeline scheduling.
//!
//! # Mental model
//!
//! * `Invocation` is input intent (`Action`, `Command(CommandInvocation)`, `Nu`).
//! * `run_invocation` is the canonical dispatcher and must be the first stop for user-triggered execution.
//! * Dispatch is staged as resolve -> policy gate -> execute -> effect flush -> deferred post hooks.
//! * `run_invocation` drains an internal queue iteratively, so Nu-generated follow-up dispatches do not recurse futures.
//! * Deferred follow-up invocations from effects/overlays/Nu schedule into the runtime work queue and are drained by runtime `drain_until_idle`.
//! * Nu post hooks are queued only for non-quit outcomes, then evaluated asynchronously and may enqueue deferred work dispatches.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`crate::types::Invocation`] | User/script invocation request | Must be dispatched only through `run_invocation` | key handling, command paths, Nu pipelines |
//! | [`crate::types::InvocationPolicy`] | Enforcement policy toggles | Enforcing mode must block capability/readonly violations | `InvocationPolicy::enforcing` |
//! | [`crate::types::InvocationOutcome`] | Canonical execution outcome | Must preserve quit/force-quit/error propagation with stable status labels | branch handlers in this module |
//! | [`engine::InvocationEngine`] | Queue-driven invocation orchestrator | Must execute frames in FIFO-with-front-insert order and short-circuit on terminal outcomes | `Editor::run_invocation` |
//! | [`engine::InvocationFrame`] | Per-step invocation envelope (`invocation`, `nu_depth`, `origin`) | Nu follow-up frames must increase `nu_depth` and preserve deterministic order | `engine::InvocationEngine::run` |
//! | [`engine::InvocationStepOutcome`] | Normalized per-step result envelope | Must carry post-hook emission data and follow-up frames explicitly | `engine::InvocationEngine::run_frame` |
//! | [`policy_gate::InvocationGateInput`] | Shared policy envelope for targets | Must carry required caps and mutability intent before execution | action/command executors |
//! | [`policy_gate::GateResult`] | Policy gate result | `Deny` must return without running target handlers | `Editor::gate_invocation` |
//! | [`kernel::InvocationKernel`] | Shared invocation executor boundary | Must centralize policy/flush/error shaping to avoid branch drift | action/command/Nu executors |
//! | [`crate::types::invocation::adapters`] | Consumer translation helpers | Must keep Nu consumers aligned on outcome mapping and logging | `commands::nu`, `nu::pipeline` |
//! | [`crate::impls::Editor`] | Runtime owner of invocation execution | Must flush queued effects after each command/action execution branch | `run_*_invocation` methods |
//! | [`crate::nu::ctx::NuCtxEvent`] | Deferred hook event payload | Must enqueue only when execution does not request quit | `run_invocation` |
//! | [`crate::runtime::work_queue::RuntimeWorkQueue`] | Runtime deferred work queue | Must preserve FIFO sequence and source metadata for policy routing | queue producers and `Editor::drain_runtime_work_report` |
//!
//! # Invariants
//!
//! * Must gate capability violations through `InvocationPolicy` before handler execution.
//! * Must gate readonly edits when policy enforces readonly and target requires edit capability.
//! * Action and command execution must pass through the shared policy gate.
//! * Command auto-route resolution must prefer editor commands before registry commands.
//! * Keymap-produced invocations must route through `run_invocation`.
//! * Must enqueue Nu post hooks only for non-quit invocation outcomes.
//! * Must cap Nu macro recursion depth to prevent unbounded self-recursion.
//! * Must flush queued effects after action/command execution branches.
//! * Deferred invocation drain must enforce source-aware policy (Nu sources enforcing, non-Nu sources log-only).
//! * Deferred invocation request queueing must preserve source/policy/scope metadata.
//! * Runtime invocation work must execute through `run_invocation` with source/scope/sequence metadata preserved in drain logging.
//!
//! # Data flow
//!
//! 1. Caller builds an [`crate::types::Invocation`] and calls `run_invocation`.
//! 2. Dispatcher resolves target definition and constructs shared policy gate metadata.
//! 3. Policy gate enforces capability/readonly checks and either denies or proceeds.
//! 4. Handler executes and returns typed outcome/effects.
//! 5. Effects are flushed and transformed into editor mutations and overlay/layer events.
//! 6. Engine converts step outcomes into explicit post-hook emissions and follow-up frames.
//! 7. Nu macro outcomes enqueue follow-up invocations into the local invocation queue as `origin=NuMacro` frames.
//! 8. Non-quit outcomes enqueue Nu post hooks.
//! 9. Effects/overlays/Nu schedule enqueue deferred invocations into runtime work queue.
//! 10. Runtime `drain_until_idle` drains queued invocation work in FIFO order with source-aware policy.
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
//! * Nu macro follow-up dispatch is iterative and depth-bounded by `MAX_NU_MACRO_DEPTH`.
//!
//! # Failure modes & recovery
//!
//! * Unknown target: return `InvocationStatus::NotFound` with canonical detail string.
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
//!   2. Route violations through the shared `policy_gate` helpers.
//!   3. Add invariant proof in `invariants.rs`.

mod dispatch;
mod engine;
mod execute_action;
mod execute_command;
mod execute_nu;
mod hooks_bridge;
mod kernel;
mod policy_gate;

#[cfg(test)]
pub(crate) use hooks_bridge::{action_post_event, command_post_event};

#[cfg(test)]
use crate::impls::Editor;
#[cfg(test)]
use crate::types::{Invocation, InvocationPolicy};

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
