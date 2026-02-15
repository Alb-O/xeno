//! Syntax manager for background parsing, scheduling, and install policy.
//! Anchor ID: XENO_ANCHOR_SYNTAX_MANAGER
//!
//! # Purpose
//!
//! Coordinate syntax parsing work across documents, balancing responsiveness and
//! resource usage with tiered policy, hotness-aware retention, and monotonic
//! install rules.
//!
//! # Mental model
//!
//! Each document is polled through an explicit pipeline per frame:
//!
//! `derive → normalize → install → gate → bootstrap → plan → spawn → finalize`
//!
//! * `derive` — compute tier, config, viewport bounds from policy + context (pure).
//! * `normalize` — reset on language/opts changes.
//! * `install` — drain completed tasks; decide install/discard/retention-drop per result.
//! * `gate` — language/disabled/ready/debounce early exits.
//! * `bootstrap` — synchronous first-visible parse for small/medium docs.
//! * `plan` — pure scheduling decisions → `WorkPlan` (Stage-A, Stage-B, BG).
//! * `spawn` — execute plan, apply side effects on successful task spawn.
//! * `finalize` — derive poll result from plan + active state.
//!
//! The render-frame workset includes visible docs, dirty docs, docs with pending
//! background tasks, and docs with unprocessed completions (`docs_with_completed`).
//! This ensures completions are installed/discarded even for docs no longer visible.
//!
//! History-urgent Stage-A retries are one-shot per viewport key and doc version
//! after timeout/error, so background catch-up can make forward progress.
//!
//! # Key types
//!
//! | Type | Role | Notes |
//! |---|---|---|
//! | [`crate::syntax_manager::SyntaxManager`] | Orchestrator | Entry point from render/tick/edit paths |
//! | [`crate::syntax_manager::SyntaxSlot`] | Tree state | Current tree, versions, pending incrementals |
//! | `DocSched` | Scheduling state | Debounce, cooldown, in-flight bookkeeping |
//! | [`crate::syntax_manager::EnsureSyntaxContext`] | Poll input | Per-document snapshot for scheduling |
//! | [`crate::syntax_manager::HighlightProjectionCtx`] | Stale highlight mapping | Bridges stale tree spans to current rope |
//!
//! # Invariants
//!
//! * Must not install parse results from older epochs.
//! * Must not regress installed tree doc version.
//! * Must keep `syntax_version` monotonic on tree install/drop.
//! * Must rotate full-tree identity when sync incremental catch-up mutates the tree.
//! * Must only expose highlight projection context when pending edits align to resident tree.
//! * Must only install stale viewport results when continuity requires filling uncovered viewports.
//! * Must skip stale non-viewport installs that would break projection continuity to the current document version.
//! * Must prefer eager urgent viewport parses after L-tier history edits, even when a full tree is present, to reduce two-step undo repaint churn.
//! * Must preserve the resident full-tree version on history edits so projection can reuse the prior syntax baseline during async catch-up.
//! * Must restore remembered full-tree snapshots immediately when history edits return to previously seen content.
//! * Must bound viewport scheduling to a capped visible byte span.
//! * Must use viewport-specific cooldowns for viewport task failures.
//! * Must suppress same-version history Stage-A retries after urgent timeout/error so background catch-up is not starved.
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
//! * Create manager once at editor startup.
//! * Poll from render loop.
//! * Drain finished tasks from tick.
//! * Remove document state on close.
//!
//! # Concurrency & ordering
//!
//! * Global semaphore enforces parse concurrency.
//! * Document epoch invalidates stale background completions.
//! * Requested document version prevents old-task flicker installs.
//! * Visible uncovered viewports may preempt tracked full/incremental work by epoch invalidation.
//!
//! # Failure modes & recovery
//!
//! * Timeouts/errors enter cooldown.
//! * Viewport task failures use short viewport cooldowns so visible recovery stays responsive.
//! * History-urgent Stage-A failures are latched per viewport key/doc-version to
//!   prevent retry loops from starving background full/incremental recovery.
//! * Retention drops trees for cold docs when configured.
//! * Incremental misalignment falls back to full reparse.
//!
//! # Recipes
//!
//! * For edit bursts: use `note_edit_incremental`, then `ensure_syntax`.
//! * For rendering stale-but-continuous highlights: use
//!   `SyntaxManager::highlight_projection_ctx_for`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;
use xeno_language::LanguageLoader;
use xeno_language::syntax::{InjectionPolicy, SyntaxOptions};
use xeno_primitives::{ChangeSet, Rope};

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
pub use types::{
	DocEpoch, EditSource, EnsureSyntaxContext, HighlightProjectionCtx, OptKey, SyntaxPollOutcome, SyntaxPollResult, SyntaxSelection, SyntaxSlot, TaskId,
	ViewportCache, ViewportEntry, ViewportKey, ViewportTree,
};
pub(crate) use types::{InstalledTree, PendingIncrementalEdits};
#[cfg(test)]
pub(crate) use xeno_language::LanguageId;

struct DocEntry {
	sched: DocSched,
	slot: SyntaxSlot,
	/// Last known tier for retention sweep (updated on each ensure_syntax call).
	last_tier: Option<policy::SyntaxTier>,
}

impl DocEntry {
	fn new(now: Instant) -> Self {
		Self {
			sched: DocSched::new(now),
			slot: SyntaxSlot::default(),
			last_tier: None,
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
