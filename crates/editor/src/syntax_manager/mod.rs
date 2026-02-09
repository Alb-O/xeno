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
//!
//! # Failure modes & recovery
//!
//! - Timeouts/errors enter cooldown.
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

mod engine;
mod policy;
mod scheduling;
mod tasks;
mod types;

use engine::RealSyntaxEngine;
pub use engine::SyntaxEngine;
pub use metrics::SyntaxMetrics;
pub use policy::{
	RetentionPolicy, SyntaxHotness, SyntaxManagerCfg, SyntaxTier, TierCfg, TieredSyntaxPolicy,
};
use scheduling::CompletedSyntaxTask;
pub(crate) use scheduling::DocSched;
pub use tasks::TaskClass;
pub(crate) use tasks::TaskCollector;
use tasks::{TaskKind, TaskSpec};
pub(crate) use types::PendingIncrementalEdits;
pub use types::{
	DocEpoch, EditSource, EnsureSyntaxContext, HighlightProjectionCtx, OptKey,
	SyntaxPollOutcome, SyntaxPollResult, SyntaxSlot, TaskId,
};
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

impl Default for SyntaxManager {
	/// Creates a new manager with default concurrency limits.
	fn default() -> Self {
		Self::new(SyntaxManagerCfg::default())
	}
}

impl SyntaxManager {
	pub fn new(cfg: SyntaxManagerCfg) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
			metrics: SyntaxMetrics::new(),
			permits: Arc::new(Semaphore::new(cfg.max_concurrency.max(1))),
			entries: HashMap::new(),
			engine: Arc::new(RealSyntaxEngine),
			collector: TaskCollector::new(),
			cfg,
		}
	}

	#[cfg(any(test, doc))]
	pub fn new_with_engine(cfg: SyntaxManagerCfg, engine: Arc<dyn SyntaxEngine>) -> Self {
		Self {
			policy: TieredSyntaxPolicy::test_default(),
			metrics: SyntaxMetrics::new(),
			permits: Arc::new(Semaphore::new(cfg.max_concurrency.max(1))),
			entries: HashMap::new(),
			engine,
			collector: TaskCollector::new(),
			cfg,
		}
	}

	/// Clears the dirty flag for a document without going through a parse cycle.
	///
	/// Test-only helper that enables sibling modules (e.g. `invariants`) to
	/// manipulate private [`SyntaxSlot`] state for edge-case coverage.
	#[cfg(test)]
	pub(crate) fn force_clean(&mut self, doc_id: DocumentId) {
		self.entry_mut(doc_id).slot.dirty = false;
	}

	pub fn set_policy(&mut self, policy: TieredSyntaxPolicy) {
		assert!(
			policy.s_max_bytes_inclusive <= policy.m_max_bytes_inclusive,
			"TieredSyntaxPolicy: s_max ({}) must be <= m_max ({})",
			policy.s_max_bytes_inclusive,
			policy.m_max_bytes_inclusive
		);
		self.policy = policy;
	}

	fn entry_mut(&mut self, doc_id: DocumentId) -> &mut DocEntry {
		self.entries
			.entry(doc_id)
			.or_insert_with(|| DocEntry::new(Instant::now()))
	}

	pub fn has_syntax(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.and_then(|e| e.slot.current.as_ref())
			.is_some()
	}

	pub fn is_dirty(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.map(|e| e.slot.dirty)
			.unwrap_or(false)
	}

	pub fn syntax_for_doc(&self, doc_id: DocumentId) -> Option<&Syntax> {
		self.entries
			.get(&doc_id)
			.and_then(|e| e.slot.current.as_ref())
	}

	pub fn syntax_version(&self, doc_id: DocumentId) -> u64 {
		self.entries
			.get(&doc_id)
			.map(|e| e.slot.version)
			.unwrap_or(0)
	}

	/// Returns the document version that the installed syntax tree corresponds to.
	#[cfg(test)]
	pub(crate) fn syntax_doc_version(&self, doc_id: DocumentId) -> Option<u64> {
		self.entries.get(&doc_id)?.slot.tree_doc_version
	}

	/// Returns projection context for mapping stale tree highlights onto current text.
	///
	/// Returns `None` when tree and target versions already match, or when no
	/// aligned pending window exists.
	pub(crate) fn highlight_projection_ctx(
		&self,
		doc_id: DocumentId,
		doc_version: u64,
	) -> Option<HighlightProjectionCtx<'_>> {
		let entry = self.entries.get(&doc_id)?;
		let tree_doc_version = entry.slot.tree_doc_version?;
		if tree_doc_version == doc_version {
			return None;
		}

		let pending = entry.slot.pending_incremental.as_ref()?;
		if pending.base_tree_doc_version != tree_doc_version {
			return None;
		}

		Some(HighlightProjectionCtx {
			tree_doc_version,
			target_doc_version: doc_version,
			base_rope: &pending.old_rope,
			composed_changes: &pending.composed,
		})
	}

	/// Returns the document-global byte coverage of the installed syntax tree.
	pub fn syntax_coverage(&self, doc_id: DocumentId) -> Option<std::ops::Range<u32>> {
		self.entries.get(&doc_id)?.slot.coverage.clone()
	}

	/// Returns true if a background task is currently active for a document (even if detached).
	#[cfg(test)]
	pub(crate) fn has_inflight_task(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.is_some_and(|e| e.sched.active_task.is_some())
	}

	/// Resets the syntax state for a document, clearing the current tree and history.
	pub fn reset_syntax(&mut self, doc_id: DocumentId) {
		let entry = self.entry_mut(doc_id);
		if entry.slot.current.is_some() {
			entry.slot.drop_tree();
			Self::mark_updated(&mut entry.slot);
		}
		entry.slot.dirty = true;
		entry.slot.pending_incremental = None;
		entry.sched.invalidate();
	}

	/// Marks a document as dirty, triggering a reparse on the next poll.
	pub fn mark_dirty(&mut self, doc_id: DocumentId) {
		self.entry_mut(doc_id).slot.dirty = true;
	}

	/// Records an edit for debounce scheduling without changeset data.
	pub fn note_edit(&mut self, doc_id: DocumentId, source: EditSource) {
		let now = Instant::now();
		let entry = self.entry_mut(doc_id);
		entry.sched.last_edit_at = now;
		entry.slot.dirty = true;
		if source == EditSource::History {
			entry.sched.force_no_debounce = true;
		}
	}

	/// Records an edit and attempts an immediate incremental update.
	///
	/// This is the primary path for interactive typing. It attempts to update
	/// the resident syntax tree synchronously (with a 10ms timeout). If the
	/// update fails or is debounced, it accumulates the changes for a
	/// background parse.
	///
	/// # Invariants
	///
	/// - Sync incremental updates are ONLY allowed if the resident tree's version
	///   matches the version immediately preceding this edit.
	/// - If alignment is lost, we fallback to a full reparse in the background.
	pub fn note_edit_incremental(
		&mut self,
		doc_id: DocumentId,
		doc_version: u64,
		old_rope: &Rope,
		new_rope: &Rope,
		changeset: &ChangeSet,
		loader: &LanguageLoader,
		source: EditSource,
	) {
		const SYNC_TIMEOUT: Duration = Duration::from_millis(10);

		let now = Instant::now();
		let entry = self.entry_mut(doc_id);
		entry.sched.last_edit_at = now;
		entry.slot.dirty = true;

		if source == EditSource::History {
			entry.sched.force_no_debounce = true;
		}

		if let Some(current) = &entry.slot.current
			&& current.is_partial()
		{
			// Never attempt incremental updates on a partial tree.
			// Drop it immediately to avoid stale highlighting.
			entry.slot.drop_tree();
			Self::mark_updated(&mut entry.slot);
			return;
		}

		let Some(syntax) = entry.slot.current.as_mut() else {
			entry.slot.pending_incremental = None;
			return;
		};

		if doc_version == 0 {
			entry.slot.pending_incremental = None;
			return;
		}
		let version_before = doc_version - 1;

		// Manage pending incremental window
		match entry.slot.pending_incremental.take() {
			Some(mut pending) => {
				if entry.slot.tree_doc_version != Some(pending.base_tree_doc_version) {
					// Tree has diverged from pending base; invalid window
					entry.slot.pending_incremental = None;
				} else {
					pending.composed = pending.composed.compose(changeset.clone());
					entry.slot.pending_incremental = Some(pending);
				}
			}
			None => {
				// Only start a pending window if the tree matches the version before this edit.
				if let Some(tree_v) = entry.slot.tree_doc_version
					&& tree_v == version_before
				{
					entry.slot.pending_incremental = Some(PendingIncrementalEdits {
						base_tree_doc_version: tree_v,
						old_rope: old_rope.clone(),
						composed: changeset.clone(),
					});
				}
			}
		}

		let Some(pending) = entry.slot.pending_incremental.as_ref() else {
			return;
		};

		// Attempt sync catch-up from pending base to latest rope
		let opts = SyntaxOptions {
			parse_timeout: SYNC_TIMEOUT,
			..syntax.opts()
		};

		if syntax
			.update_from_changeset(
				pending.old_rope.slice(..),
				new_rope.slice(..),
				&pending.composed,
				loader,
				opts,
			)
			.is_ok()
		{
			entry.slot.pending_incremental = None;
			entry.slot.dirty = false;
			entry.slot.tree_doc_version = Some(doc_version);
			Self::mark_updated(&mut entry.slot);
		} else {
			tracing::debug!(
				?doc_id,
				"Sync incremental update failed; keeping pending for catch-up"
			);
		}
	}

	/// Cleans up tracking state for a closed document.
	pub fn on_document_close(&mut self, doc_id: DocumentId) {
		self.forget_doc(doc_id);
	}

	/// Removes all tracking state and pending tasks for a document.
	pub fn forget_doc(&mut self, doc_id: DocumentId) {
		if let Some(mut entry) = self.entries.remove(&doc_id) {
			entry.sched.invalidate();
		}
	}

	pub fn has_pending(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.is_some_and(|d| d.sched.active_task.is_some() && !d.sched.active_task_detached)
	}

	pub fn pending_count(&self) -> usize {
		self.entries
			.values()
			.filter(|d| d.sched.active_task.is_some() && !d.sched.active_task_detached)
			.count()
	}

	pub fn pending_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.entries
			.iter()
			.filter(|(_, d)| d.sched.active_task.is_some() && !d.sched.active_task_detached)
			.map(|(id, _)| *id)
	}

	pub fn dirty_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.entries
			.iter()
			.filter(|(_, e)| e.slot.dirty)
			.map(|(id, _)| *id)
	}

	/// Returns true if any background task has completed its work.
	pub fn any_task_finished(&self) -> bool {
		self.collector.any_finished()
	}

	/// Drains all completed background tasks and installs results if valid.
	pub fn drain_finished_inflight(&mut self) -> bool {
		let mut any_drained = false;
		let results = self.collector.drain_finished();

		for res in results {
			if let Some(entry) = self.entries.get_mut(&res.doc_id) {
				// Clear active_task if it matches the one that just finished, regardless of epoch
				if entry.sched.active_task == Some(res.id) {
					entry.sched.active_task = None;
					entry.sched.active_task_detached = false;
				}

				// Epoch check: discard stale results
				if entry.sched.epoch != res.epoch {
					continue;
				}

				entry.sched.completed = Some(CompletedSyntaxTask {
					doc_version: res.doc_version,
					lang_id: res.lang_id,
					opts: res.opts_key,
					result: res.result,
					class: res.class,
					injections: res.injections,
					elapsed: res.elapsed,
				});
				any_drained = true;
			}
		}
		any_drained
	}

	/// Invariant enforcement: Polls or kicks background syntax parsing for a document.
	pub fn ensure_syntax(&mut self, ctx: EnsureSyntaxContext<'_>) -> SyntaxPollOutcome {
		let now = Instant::now();
		let doc_id = ctx.doc_id;

		// Calculate policy and options key
		let bytes = ctx.content.len_bytes();
		let tier = self.policy.tier_for_bytes(bytes);
		let cfg = self.policy.cfg(tier);
		let current_opts_key = OptKey {
			injections: cfg.injections,
		};

		let work_disabled = matches!(ctx.hotness, SyntaxHotness::Cold) && !cfg.parse_when_hidden;

		// 1. Initial entry check & change detection
		let mut was_updated = {
			let entry = self.entry_mut(doc_id);
			let mut updated = entry.slot.take_updated();

			if entry.slot.language_id != ctx.language_id {
				entry.sched.invalidate();
				if entry.slot.current.is_some() {
					entry.slot.drop_tree();
					Self::mark_updated(&mut entry.slot);
					updated = true;
				}
				entry.slot.language_id = ctx.language_id;
			}

			if matches!(ctx.hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
				entry.sched.last_visible_at = now;
			}

			if entry
				.slot
				.last_opts_key
				.is_some_and(|k| k != current_opts_key)
			{
				entry.sched.invalidate();
				entry.slot.dirty = true;
				entry.slot.drop_tree();
				Self::mark_updated(&mut entry.slot);
				updated = true;
			}
			entry.slot.last_opts_key = Some(current_opts_key);

			if entry.sched.active_task.is_some() {
				entry.sched.active_task_detached = work_disabled;
			}

			updated
		};

		// 2. Process completed tasks (from local cache)
		let task_record = {
			let entry = self.entry_mut(doc_id);
			if let Some(done) = entry.sched.completed.take() {
				let lang_id = done.lang_id;
				let class = done.class;
				let injections = done.injections;
				let elapsed = done.elapsed;
				let is_timeout = matches!(
					done.result,
					Err(xeno_runtime_language::syntax::SyntaxError::Timeout)
				);
				let is_error = done.result.is_err() && !is_timeout;

				match done.result {
					Ok(syntax_tree) => {
						if let Some(current_lang) = ctx.language_id {
							let lang_ok = lang_id == current_lang;
							let opts_ok = if class == TaskClass::Viewport {
								let stage_a_ok = injections == cfg.viewport_injections;
								let stage_b_ok = injections == InjectionPolicy::Eager
									&& cfg.viewport_stage_b_budget.is_some();
								stage_a_ok || stage_b_ok
							} else {
								done.opts == current_opts_key
							};
							let version_match = done.doc_version == ctx.doc_version;

							let retain_ok = Self::retention_allows_install(
								now,
								&entry.sched,
								cfg.retention_hidden,
								ctx.hotness,
							);

							let allow_install = if class == TaskClass::Viewport {
								done.doc_version == ctx.doc_version
							} else {
								Self::should_install_completed_parse(
									done.doc_version,
									entry.slot.tree_doc_version,
									entry.sched.requested_doc_version,
									ctx.doc_version,
									entry.slot.dirty,
								)
							};

							let is_installed = if work_disabled {
								tracing::trace!(
									?doc_id,
									"Discarding syntax result because work is disabled (Cold)"
								);
								false
							} else if lang_ok && opts_ok && retain_ok && allow_install {
								let is_viewport = class == TaskClass::Viewport;
								if is_viewport {
									entry.slot.dirty = true;
									entry.sched.force_no_debounce = true;
								}

								if let Some(meta) = &syntax_tree.viewport {
									entry.slot.coverage = Some(
										meta.base_offset
											..meta.base_offset.saturating_add(meta.real_len),
									);
								} else {
									entry.slot.coverage = None;
								}

								entry.slot.current = Some(syntax_tree);
								entry.slot.language_id = Some(current_lang);
								entry.slot.tree_doc_version = Some(done.doc_version);
								entry.slot.pending_incremental = None;
								Self::mark_updated(&mut entry.slot);
								was_updated = true;

								if !is_viewport {
									entry.sched.force_no_debounce = false;
									if version_match {
										entry.slot.dirty = false;
										entry.sched.cooldown_until = None;
									} else {
										entry.slot.dirty = true;
									}
								}
								true
							} else {
								tracing::trace!(
									?doc_id,
									?lang_ok,
									?opts_ok,
									?retain_ok,
									?allow_install,
									"Discarding syntax result due to mismatch/retention"
								);
								if lang_ok && opts_ok && version_match && !retain_ok {
									entry.slot.drop_tree();
									entry.slot.dirty = false;
									entry.sched.force_no_debounce = false;
									Self::mark_updated(&mut entry.slot);
									was_updated = true;
								}
								false
							};

							Some((
								lang_id,
								tier,
								class,
								injections,
								elapsed,
								is_timeout,
								is_error,
								is_installed,
							))
						} else {
							Some((
								lang_id, tier, class, injections, elapsed, is_timeout, is_error,
								false,
							))
						}
					}
					Err(xeno_runtime_language::syntax::SyntaxError::Timeout) => {
						entry.sched.cooldown_until = Some(now + cfg.cooldown_on_timeout);
						Some((
							lang_id, tier, class, injections, elapsed, true, false, false,
						))
					}
					Err(e) => {
						tracing::warn!(?doc_id, ?tier, error=%e, "Background syntax parse failed");
						entry.sched.cooldown_until = Some(now + cfg.cooldown_on_error);
						Some((
							lang_id, tier, class, injections, elapsed, false, true, false,
						))
					}
				}
			} else {
				None
			}
		};

		if let Some((
			lang_id,
			tier,
			class,
			injections,
			elapsed,
			is_timeout,
			is_error,
			is_installed,
		)) = task_record
		{
			self.metrics.record_task_result(
				lang_id,
				tier,
				class,
				injections,
				elapsed,
				is_timeout,
				is_error,
				is_installed,
			);
			if is_timeout || is_error {
				return SyntaxPollOutcome {
					result: SyntaxPollResult::CoolingDown,
					updated: was_updated,
				};
			}
		}

		{
			let entry = self.entry_mut(doc_id);

			if entry.sched.active_task.is_some() && !entry.sched.active_task_detached {
				if Self::apply_retention(
					now,
					&entry.sched,
					cfg.retention_hidden,
					ctx.hotness,
					&mut entry.slot,
					doc_id,
				) {
					entry.sched.invalidate();
					was_updated = true;
				} else {
					return SyntaxPollOutcome {
						result: SyntaxPollResult::Pending,
						updated: was_updated,
					};
				}
			}

			// 4. Language check
			let Some(_lang_id) = ctx.language_id else {
				if entry.slot.current.is_some() {
					entry.slot.drop_tree();
					Self::mark_updated(&mut entry.slot);
					was_updated = true;
				}
				entry.slot.language_id = None;
				entry.slot.dirty = false;
				entry.sched.cooldown_until = None;
				return SyntaxPollOutcome {
					result: SyntaxPollResult::NoLanguage,
					updated: was_updated,
				};
			};

			if Self::apply_retention(
				now,
				&entry.sched,
				cfg.retention_hidden,
				ctx.hotness,
				&mut entry.slot,
				doc_id,
			) {
				if !work_disabled {
					entry.sched.invalidate();
				}
				was_updated = true;
			}

			// 5. Gating
			if work_disabled {
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Disabled,
					updated: was_updated,
				};
			}

			if entry.slot.current.is_some() && !entry.slot.dirty {
				entry.sched.force_no_debounce = false;
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Ready,
					updated: was_updated,
				};
			}

			if entry.slot.current.is_some()
				&& !entry.sched.force_no_debounce
				&& now.duration_since(entry.sched.last_edit_at) < cfg.debounce
			{
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Pending,
					updated: was_updated,
				};
			}

			if let Some(until) = entry.sched.cooldown_until
				&& now < until
			{
				return SyntaxPollOutcome {
					result: SyntaxPollResult::CoolingDown,
					updated: was_updated,
				};
			}

			// Defensive: never schedule if a task is already active for this document identity
			if let Some(_task_id) = entry.sched.active_task
				&& !entry.sched.active_task_detached
			{
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Pending,
					updated: was_updated,
				};
			}
		}

		// 5.5 Sync bootstrap fast path
		let lang_id = ctx.language_id.unwrap();
		let (do_sync, sync_timeout, pre_epoch) = {
			let entry = self.entry_mut(doc_id);
			let is_bootstrap = entry.slot.current.is_none();
			let is_visible = matches!(ctx.hotness, SyntaxHotness::Visible);

			if is_bootstrap && is_visible && !entry.slot.sync_bootstrap_attempted {
				if let Some(t) = cfg.sync_bootstrap_timeout {
					entry.slot.sync_bootstrap_attempted = true;
					(true, Some(t), entry.sched.epoch)
				} else {
					(false, None, entry.sched.epoch)
				}
			} else {
				(false, None, entry.sched.epoch)
			}
		};

		let sync_result = if do_sync {
			let sync_opts = SyntaxOptions {
				parse_timeout: sync_timeout.unwrap(),
				injections: cfg.injections,
			};
			Some(
				self.engine
					.parse(ctx.content.slice(..), lang_id, ctx.loader, sync_opts),
			)
		} else {
			None
		};

		if let Some(res) = sync_result
			&& let Ok(syntax) = res
		{
			let entry = self.entry_mut(doc_id);
			// Re-check invariants after dropping and re-acquiring borrow
			let is_bootstrap = entry.slot.current.is_none();
			let is_visible = matches!(ctx.hotness, SyntaxHotness::Visible);
			if entry.sched.epoch == pre_epoch
				&& is_bootstrap
				&& is_visible
				&& entry.sched.active_task.is_none()
			{
				entry.slot.current = Some(syntax);
				entry.slot.language_id = Some(lang_id);
				entry.slot.tree_doc_version = Some(ctx.doc_version);
				entry.slot.dirty = false;
				entry.slot.coverage = None;
				entry.slot.viewport_stage_b_attempted = false;
				entry.slot.pending_incremental = None;
				entry.sched.force_no_debounce = false;
				entry.sched.cooldown_until = None;
				Self::mark_updated(&mut entry.slot);
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Ready,
					updated: true,
				};
			}
		}

		// 6. Schedule new task
		// 6a. Viewport-bounded parsing (Stage A and Stage B)
		let (needs_stage_a, needs_stage_b, win_override) = {
			let entry = self.entry_mut(doc_id);
			let mut needs_a = false;
			let mut needs_b = false;
			let mut win = None;

			if tier == SyntaxTier::L
				&& ctx.hotness == SyntaxHotness::Visible
				&& let Some(viewport) = &ctx.viewport
			{
				if let Some(current) = &entry.slot.current {
					if current.is_partial() {
						if let Some(coverage) = &entry.slot.coverage {
							if viewport.start < coverage.start || viewport.end > coverage.end {
								needs_a = true;
							} else if current.opts().injections != InjectionPolicy::Eager
								&& !entry.slot.viewport_stage_b_attempted
								&& cfg.viewport_stage_b_budget.is_some()
							{
								needs_b = true;
								win = Some(coverage.clone());
							}
						} else {
							needs_a = true;
						}
					}
				} else {
					needs_a = true;
				}
			}
			(needs_a, needs_b, win)
		};

		let mut really_needs_b = false;
		if needs_stage_b {
			let budget = cfg.viewport_stage_b_budget.unwrap();
			let predicted = self.metrics.predict_duration(
				lang_id,
				tier,
				TaskClass::Viewport,
				InjectionPolicy::Eager,
			);

			// If no metrics yet, we assume it's within budget (optimistic)
			if predicted.map(|p| p <= budget).unwrap_or(true) {
				really_needs_b = true;
			}
		}

		if (needs_stage_a || really_needs_b) && self.entry_mut(doc_id).sched.active_task.is_none() {
			let mut b_latch_to_set = false;
			let (injections, win_start, win_end) = {
				let entry = self.entry_mut(doc_id);
				let viewport = ctx.viewport.as_ref().unwrap();

				if needs_stage_a {
					if entry.slot.current.as_ref().is_some_and(|s| s.is_partial()) {
						entry.slot.drop_tree();
						Self::mark_updated(&mut entry.slot);
					}

					let win_start = viewport.start.saturating_sub(cfg.viewport_lookbehind);
					let mut win_end = (viewport.end + cfg.viewport_lookahead).min(bytes as u32);
					let mut win_len = win_end.saturating_sub(win_start);

					if win_len > cfg.viewport_window_max {
						win_len = cfg.viewport_window_max;
						win_end = (win_start + win_len).min(bytes as u32);
					}
					(cfg.viewport_injections, win_start, win_end)
				} else {
					// needs_stage_b
					b_latch_to_set = true;
					let range = win_override.as_ref().unwrap();
					(InjectionPolicy::Eager, range.start, range.end)
				}
			};

			if win_end > win_start {
				let class = TaskClass::Viewport;
				let parse_timeout = self.metrics.derive_timeout(
					lang_id,
					tier,
					class,
					injections,
					cfg.viewport_parse_timeout_min,
					cfg.viewport_parse_timeout_max,
				);

				let entry = self.entry_mut(doc_id);
				let spec = TaskSpec {
					doc_id,
					epoch: entry.sched.epoch,
					doc_version: ctx.doc_version,
					lang_id,
					opts_key: current_opts_key,
					opts: SyntaxOptions {
						parse_timeout,
						injections,
					},
					kind: TaskKind::ViewportParse {
						content: ctx.content.clone(),
						window: win_start..win_end,
					},
					loader: Arc::clone(ctx.loader),
				};

				let permits = Arc::clone(&self.permits);
				let engine = Arc::clone(&self.engine);

				if let Some(task_id) =
					self.collector
						.spawn(permits, engine, spec, self.cfg.viewport_reserve)
				{
					let entry = self.entries.get_mut(&doc_id).unwrap();
					entry.sched.active_task = Some(task_id);
					entry.sched.requested_doc_version = ctx.doc_version;
					entry.sched.force_no_debounce = false;
					if b_latch_to_set {
						entry.slot.viewport_stage_b_attempted = true;
					}
					return SyntaxPollOutcome {
						result: SyntaxPollResult::Kicked,
						updated: was_updated,
					};
				}
			}
		}

		// 6b. Full or Incremental parse
		let (kind, class) = {
			let entry = self.entry_mut(doc_id);
			let incremental = match entry.slot.pending_incremental.as_ref() {
				Some(pending)
					if entry.slot.current.is_some()
						&& entry.slot.tree_doc_version == Some(pending.base_tree_doc_version) =>
				{
					Some(TaskKind::Incremental {
						base: entry.slot.current.as_ref().unwrap().clone(),
						old_rope: pending.old_rope.clone(),
						new_rope: ctx.content.clone(),
						composed: pending.composed.clone(),
					})
				}
				_ => None,
			};

			let kind = incremental.unwrap_or_else(|| TaskKind::FullParse {
				content: ctx.content.clone(),
			});
			let class = kind.class();
			(kind, class)
		};

		let injections = cfg.injections;
		let parse_timeout = self.metrics.derive_timeout(
			lang_id,
			tier,
			class,
			injections,
			cfg.parse_timeout_min,
			cfg.parse_timeout_max,
		);

		let entry = self.entry_mut(doc_id);
		let spec = TaskSpec {
			doc_id,
			epoch: entry.sched.epoch,
			doc_version: ctx.doc_version,
			lang_id,
			opts_key: current_opts_key,
			opts: SyntaxOptions {
				parse_timeout,
				injections,
			},
			kind,
			loader: Arc::clone(ctx.loader),
		};

		let permits = Arc::clone(&self.permits);
		let engine = Arc::clone(&self.engine);

		if let Some(task_id) =
			self.collector
				.spawn(permits, engine, spec, self.cfg.viewport_reserve)
		{
			let entry = self.entry_mut(doc_id);
			entry.sched.active_task = Some(task_id);
			entry.sched.requested_doc_version = ctx.doc_version;
			entry.sched.force_no_debounce = false;
			SyntaxPollOutcome {
				result: SyntaxPollResult::Kicked,
				updated: was_updated,
			}
		} else {
			SyntaxPollOutcome {
				result: SyntaxPollResult::Throttled,
				updated: was_updated,
			}
		}
	}

	/// Invariant enforcement: Checks if a completed background parse should be installed into a slot.
	pub(crate) fn should_install_completed_parse(
		done_version: u64,
		current_tree_version: Option<u64>,
		requested_version: u64,
		target_version: u64,
		slot_dirty: bool,
	) -> bool {
		if let Some(v) = current_tree_version
			&& done_version < v
		{
			return false;
		}

		// Never install results older than the last request to avoid "flicker"
		// where an old slow parse finishes after a newer faster parse was already requested.
		if done_version < requested_version {
			return false;
		}

		let version_match = done_version == target_version;
		let has_current = current_tree_version.is_some();

		version_match || slot_dirty || !has_current
	}

	/// Evaluates if the retention policy allows installing a new syntax tree.
	fn retention_allows_install(
		now: Instant,
		st: &DocSched,
		policy: RetentionPolicy,
		hotness: SyntaxHotness,
	) -> bool {
		if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
			return true;
		}
		match policy {
			RetentionPolicy::Keep => true,
			RetentionPolicy::DropWhenHidden => false,
			RetentionPolicy::DropAfter(ttl) => now.duration_since(st.last_visible_at) <= ttl,
		}
	}

	/// Invariant enforcement: Bumps the syntax version after a state change.
	pub(crate) fn mark_updated(state: &mut SyntaxSlot) {
		state.updated = true;
		state.version = state.version.wrapping_add(1);
	}

	/// Invariant enforcement: Applies memory retention rules to a syntax slot.
	pub(crate) fn apply_retention(
		now: Instant,
		st: &DocSched,
		policy: RetentionPolicy,
		hotness: SyntaxHotness,
		state: &mut SyntaxSlot,
		_doc_id: DocumentId,
	) -> bool {
		if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
			return false;
		}

		match policy {
			RetentionPolicy::Keep => false,
			RetentionPolicy::DropWhenHidden => {
				if state.current.is_some() || state.dirty {
					state.drop_tree();
					state.dirty = false;
					state.pending_incremental = None;
					Self::mark_updated(state);
					true
				} else {
					false
				}
			}
			RetentionPolicy::DropAfter(ttl) => {
				if (state.current.is_some() || state.dirty)
					&& now.duration_since(st.last_visible_at) > ttl
				{
					state.drop_tree();
					state.dirty = false;
					Self::mark_updated(state);
					true
				} else {
					false
				}
			}
		}
	}
}

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
