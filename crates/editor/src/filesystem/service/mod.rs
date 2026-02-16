//! Filesystem indexing/search service coordinator.
//! Anchor ID: XENO_ANCHOR_FILESYSTEM_SERVICE
//!
//! # Purpose
//!
//! * Owns filesystem index/search worker channel lifecycle and generation tracking.
//! * Merges index updates into local snapshots and forwards deltas to search workers.
//! * Exposes query, progress, and result surfaces to overlay controllers.
//!
//! # Mental model
//!
//! * `FsService` is a generation-scoped router:
//!   * generation increments on each re-index request.
//!   * stale worker messages are ignored by generation mismatch.
//! * Index and search workers are decoupled:
//!   * index worker discovers files and emits updates.
//!   * search worker evaluates query results against indexed data.
//! * `pump()` is the only ingestion path for worker outputs.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`FsService`] | Coordinator state and worker endpoints | Must advance generations atomically and ignore stale messages | this module |
//! | `IndexSpec` | Active index configuration identity | Must restart workers when root/options change | `ensure_index` |
//! | [`super::types::IndexMsg`] | Index worker output | Must be generation-filtered | `apply_index_msg` |
//! | [`super::types::SearchMsg`] | Search worker output | Must be generation-filtered | `apply_search_msg` |
//! | [`super::types::PumpBudget`] | Polling budget | Must bound per-cycle message processing for responsiveness | `pump` |
//!
//! # Invariants
//!
//! * Must ignore stale index/search messages whose generation differs from active generation.
//! * Must reset query/result/progress state when beginning a new generation.
//! * Must publish query IDs monotonically per generation.
//! * Must forward index deltas to search worker when search worker is active.
//! * Must stop search/index channels on `stop_index`.
//!
//! # Data flow
//!
//! 1. Caller invokes `ensure_index(root, options)`.
//! 2. Service starts new generation, spawns index and search workers, stores channels.
//! 3. Runtime calls `pump()` repeatedly with budgets.
//! 4. `pump()` drains index/search messages and applies generation-filtered updates.
//! 5. Overlay/query consumers read `progress()`, `results()`, and `result_query()`.
//!
//! # Lifecycle
//!
//! * Create with `FsService::new`.
//! * Call `ensure_index` to initialize worker generation.
//! * Repeatedly call `query` and `pump` during runtime.
//! * Call `stop_index` to terminate worker channels.
//!
//! # Concurrency & ordering
//!
//! * Workers run on background threads and communicate via MPSC channels.
//! * Service state mutation is single-threaded via editor runtime `pump`.
//! * Ordering is best-effort by receive order within each channel; generation gate ensures stale safety.
//!
//! # Failure modes & recovery
//!
//! * Worker disconnect: corresponding receiver is dropped; service continues with remaining state.
//! * Stale messages: ignored by generation checks.
//! * Search/index startup replacement: old generation data is cleared and replaced.
//! * Index worker errors: logged and ignored unless generation matches and state update is needed.
//!
//! # Recipes
//!
//! * Restart index with new root/options:
//!   1. Call `ensure_index`.
//!   2. Observe generation bump and cleared result/progress state.
//!   3. Continue pumping.
//! * Add new worker message type:
//!   1. Extend `IndexMsg` or `SearchMsg`.
//!   2. Handle in `apply_index_msg`/`apply_search_msg` with generation checks.
//!   3. Add invariant proof in `invariants.rs`.

mod core;

pub use core::FsService;

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
