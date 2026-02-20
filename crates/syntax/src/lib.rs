#![cfg_attr(test, allow(unused_crate_dependencies))]
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
//! * `install` — drain completions; evaluate policy (pure) then apply install/drop/cooldown actions.
//! * `gate` — language/disabled/ready/debounce early exits.
//! * `bootstrap` — synchronous first-visible parse for small/medium docs.
//! * `plan` — pure lane-specific scheduling decisions → `PlanSet` (Stage-A, Stage-B, BG).
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
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`crate::SyntaxManager`] | Orchestrator | Must route every poll through the derive→finalize pipeline | `SyntaxManager::ensure_syntax` |
//! | [`crate::SyntaxSlot`] | Per-document tree state | Must keep installed tree and `syntax_version` monotonic | `SyntaxManager::ensure_syntax`, `SyntaxManager::note_edit_incremental` |
//! | `DocSched` | Per-document scheduling state | Must track cooldown/debounce/in-flight lanes without permit leaks | `ensure::plan::compute_plan`, `ensure::plan::spawn_plan` |
//! | [`crate::EnsureSyntaxContext`] | Poll input snapshot | Must represent a single coherent doc/version/language view | callsites in render/tick paths |
//! | [`crate::HighlightProjectionCtx`] | Stale highlight projection context | Must be exposed only when pending edits align to resident tree | `SyntaxManager::highlight_projection_ctx_for` |
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
//! * Must run bootstrap/plan/finalize only after language/work-disabled gate success.
//! * Must run viewport-specific lane planning only when a normalized viewport is present.
//! * Must suppress Stage-B planning within a poll when Stage-A is already planned.
//! * Must silently drop late completions for closed documents without reinstalling state or leaking permits.
//! * Must release parse permits via RAII when tasks complete, even after document close.
//! * Must discard pre-switch language completions via epoch invalidation on language change.
//!
//! # Data flow
//!
//! 1. Edit path calls `note_edit`/`note_edit_incremental`.
//! 2. Render/tick path calls `ensure_syntax` with current snapshot.
//! 3. Background tasks complete and are drained.
//! 4. Completion policy is evaluated, then install/drop/cooldown actions are applied.
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
use xeno_primitives::{ChangeSet, DocumentId, Rope};

pub mod highlight_cache;
pub mod lru;
mod metrics;

mod completion;
mod core;
mod edits;
mod engine;
mod ensure;
mod manager_state;
mod policy;
mod queries;
mod retention;
mod scheduling;
mod tasks;
mod types;
use engine::RealSyntaxEngine;
pub use engine::SyntaxEngine;
pub use highlight_cache::{HighlightSpanQuery, HighlightTiles};
pub use lru::RecentDocLru;
use manager_state::DocEntry;
pub use manager_state::SyntaxManager;
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

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
