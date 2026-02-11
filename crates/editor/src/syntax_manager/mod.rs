//! Syntax manager for background parsing, scheduling, and install policy.
//!
//! # Purpose
//!
//! Coordinate syntax parsing work across documents, balancing responsiveness and
//! resource usage with tiered policy, hotness-aware retention, and monotonic
//! install rules.
//!
//! # Mental model
//!
//! The manager is a per-document state machine:
//! - `Dirty` documents need catch-up.
//! - scheduling state decides `Pending/Kicked/Ready` outcomes.
//! - completed tasks are installed only if epoch/version/retention rules allow.
//! - highlight rendering may project stale tree spans through pending edits.
//!
//! # Key types
//!
//! | Type | Role | Notes |
//! |---|---|---|
//! | [`crate::syntax_manager::SyntaxManager`] | Orchestrator | Entry point from render/tick/edit paths |
//! | [`crate::syntax_manager::SyntaxSlot`] | Tree state | Current tree, versions, pending incrementals |
//! | [`crate::syntax_manager::DocSched`] | Scheduling state | Debounce, cooldown, in-flight bookkeeping |
//! | [`crate::syntax_manager::EnsureSyntaxContext`] | Poll input | Per-document snapshot for scheduling |
//! | [`crate::syntax_manager::HighlightProjectionCtx`] | Stale highlight mapping | Bridges stale tree spans to current rope |
//!
//! # Invariants
//!
//! - Must not install parse results from older epochs.
//! - Must not regress installed tree doc version.
//! - Must keep `syntax_version` monotonic on tree install/drop.
//! - Must only expose highlight projection context when pending edits align to resident tree.
//! - Must bound viewport scheduling to a capped visible byte span.
//! - Must use viewport-specific cooldowns for viewport task failures.
//!
//! # Data flow
//!
//! 1. Edit path calls `note_edit`/`note_edit_incremental`.
//! 2. Render/tick path calls `ensure_syntax` with current snapshot.
//! 3. Background tasks complete and are drained.
//! 4. Install policy accepts or discards completion.
//! 5. Render uses tree and optional projection context for highlighting.
//!
//! # Lifecycle
//!
//! - Create manager once at editor startup.
//! - Poll from render loop.
//! - Drain finished tasks from tick.
//! - Remove document state on close.
//!
//! # Concurrency & ordering
//!
//! - Global semaphore enforces parse concurrency.
//! - Document epoch invalidates stale background completions.
//! - Requested document version prevents old-task flicker installs.
//! - Visible uncovered viewports may preempt tracked full/incremental work by epoch invalidation.
//!
//! # Failure modes & recovery
//!
//! - Timeouts/errors enter cooldown.
//! - Viewport task failures use short viewport cooldowns so visible recovery stays responsive.
//! - Retention drops trees for cold docs when configured.
//! - Incremental misalignment falls back to full reparse.
//!
//! # Recipes
//!
//! - For edit bursts: use `note_edit_incremental`, then `ensure_syntax`.
//! - For rendering stale-but-continuous highlights: use
//!   [`crate::syntax_manager::SyntaxManager::highlight_projection_ctx`].

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;
use xeno_primitives::{ChangeSet, Rope};
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::syntax::{InjectionPolicy, Syntax, SyntaxOptions};

use crate::core::document::DocumentId;

pub mod lru;
mod metrics;

mod completion;
mod core;
mod edits;
mod engine;
mod ensure;
mod policy;
mod queries;
mod retention;
mod scheduling;
mod tasks;
mod types;

use engine::RealSyntaxEngine;
pub use engine::SyntaxEngine;
pub use metrics::SyntaxMetrics;
pub use policy::{RetentionPolicy, SyntaxHotness, SyntaxManagerCfg, SyntaxTier, TierCfg, TieredSyntaxPolicy};
use scheduling::CompletedSyntaxTask;
pub(crate) use scheduling::DocSched;
pub use tasks::TaskClass;
pub(crate) use tasks::TaskCollector;
use tasks::{TaskKind, TaskSpec};
pub(crate) use types::PendingIncrementalEdits;
pub use types::{DocEpoch, EditSource, EnsureSyntaxContext, HighlightProjectionCtx, OptKey, SyntaxPollOutcome, SyntaxPollResult, SyntaxSlot, TaskId};
#[cfg(test)]
pub(crate) use xeno_runtime_language::LanguageId;

struct DocEntry {
	sched: DocSched,
	slot: SyntaxSlot,
}

impl DocEntry {
	fn new(now: Instant) -> Self {
		Self {
			sched: DocSched::new(now),
			slot: SyntaxSlot::default(),
		}
	}
}

/// Top-level scheduler for background syntax parsing and results storage.
///
/// The [`SyntaxManager`] enforces global concurrency limits via a semaphore and
/// manages per-document state, including incremental updates and tiered policies.
/// It integrates with the editor tick and render loops to ensure monotonic tree
/// installation and prompt permit release.
pub struct SyntaxManager {
	/// Global configuration.
	cfg: SyntaxManagerCfg,
	/// Tiered policy mapping file size to specific configurations.
	policy: TieredSyntaxPolicy,
	/// Runtime metrics for adaptive scheduling.
	metrics: SyntaxMetrics,
	/// Global semaphore limiting concurrent background parse tasks.
	permits: Arc<Semaphore>,
	/// Per-document scheduling and syntax state.
	entries: HashMap<DocumentId, DocEntry>,
	/// Pluggable parsing engine (abstracted for tests).
	engine: Arc<dyn SyntaxEngine>,
	/// Collector for background tasks.
	collector: TaskCollector,
}

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
