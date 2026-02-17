//! Filesystem indexing/search service actor coordinator.
//! Anchor ID: XENO_ANCHOR_FILESYSTEM_SERVICE
//!
//! # Purpose
//!
//! * Owns filesystem service/index/search actor topology and generation tracking.
//! * Merges index updates into local snapshots and forwards deltas to search actors.
//! * Exposes query, progress, and result surfaces to overlay controllers.
//!
//! # Mental model
//!
//! * `FsService` is a generation-scoped command handle:
//!   * generation increments on each re-index request.
//!   * stale worker messages are ignored by generation mismatch.
//! * Actor graph:
//!   * `fs.service` owns authoritative state.
//!   * `fs.indexer` executes indexing workflow and emits typed updates.
//!   * `fs.search` owns corpus updates and query execution directly.
//! * Worker outputs are pushed into `fs.service` as typed events.
//! * Consumers drain pushed snapshot events from `FsService` and refresh UI from snapshots.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`FsService`] | Handle + observable snapshot cache | Must enqueue commands and expose latest actor state | this module |
//! | `FsServiceActor` | Authoritative state machine | Must apply generation filtering and publish snapshots | `core.rs` |
//! | `FsServiceCmd` | Service actor command protocol | Must multiplex editor commands and child worker events through one ingress queue | `core.rs` |
//! | `FsIndexerCmd`/`FsIndexerEvt` | Index actor protocol | Must forward index outputs to `fs.service` | `core.rs` |
//! | `FsSearchCmd`/`FsSearchEvt` | Search actor protocol | Must forward search outputs to `fs.service` | `core.rs` |
//! | `IndexSpec` | Active index configuration identity | Must restart workers when root/options change | `ensure_index` |
//! | [`super::types::IndexMsg`] | Index worker output | Must be generation-filtered | `apply_index_msg` |
//! | [`super::types::SearchMsg`] | Search worker output | Must be generation-filtered | `apply_search_msg` |
//! | `FsServiceEvt::SnapshotChanged` | Snapshot push event stream | Must emit when observable snapshots change | `handle` |
//! | `core::FsServiceShutdownReport` | Coordinated actor shutdown status | Must report service/index/search stop outcomes | `shutdown` |
//!
//! # Invariants
//!
//! * Must ignore stale index/search messages whose generation differs from active generation.
//! * Must reset query/result/progress state when beginning a new generation.
//! * Must keep query ID sequencing actor-owned and internal to `FsServiceActor`.
//! * Must forward index deltas to search actor when search actor is active.
//! * Must emit `SnapshotChanged` when shared snapshots mutate.
//!
//! # Data flow
//!
//! 1. Caller invokes `ensure_index(root, options)`.
//! 2. `FsService` enqueues command to `fs.service`.
//! 3. `fs.service` starts/restarts `fs.indexer` and `fs.search` for the new generation.
//! 4. Child actors forward worker outputs to `fs.service` as typed events.
//! 5. `fs.service` applies generation-filtered updates, publishes snapshots, and flips the changed flag.
//! 6. Overlay/query consumers read `progress()`, `results()`, and `result_query()`.
//!
//! # Lifecycle
//!
//! * Create with `FsService::new`.
//! * Call `ensure_index` to initialize worker generation.
//! * Call `query` as user input changes.
//! * Call `drain_events` to consume push notifications and refresh UI from snapshots.
//! * Call `shutdown` with explicit mode for bounded teardown.
//!
//! # Concurrency & ordering
//!
//! * Service/index/search actors process commands sequentially per mailbox.
//! * Worker events and handle commands share one ingress queue into the service actor.
//! * Ordering is best-effort by mailbox receive order; generation gate ensures stale safety.
//!
//! # Failure modes & recovery
//!
//! * Child actor failure: supervised restart policy respawns actor.
//! * Command channel disconnect: dispatcher exits and service stops accepting updates.
//! * Stale messages: ignored by generation checks.
//! * Search/index startup replacement: old generation data is cleared and replaced.
//! * Index worker errors: logged and ignored unless generation matches and state update is needed.
//!
//! # Recipes
//!
//! * Restart index with new root/options:
//!   1. Call `ensure_index`.
//!   2. Observe generation bump and cleared result/progress state.
//!   3. Continue reading snapshots and draining push events via `drain_events`.
//! * Add new worker message type:
//!   1. Extend `IndexMsg` or `SearchMsg`.
//!   2. Handle in `apply_index_msg`/`apply_search_msg` with generation checks.
//!   3. Add invariant proof in `invariants.rs`.

mod core;

pub(crate) use core::FsService;

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
