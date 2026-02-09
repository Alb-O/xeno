use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;
use xeno_primitives::{ChangeSet, Rope};
use xeno_runtime_language::LanguageLoader;
use xeno_runtime_language::syntax::{InjectionPolicy, Syntax, SyntaxOptions};

use crate::core::document::DocumentId;

pub mod lru;

mod engine;
mod policy;
mod scheduling;
mod tasks;
mod types;

use engine::RealSyntaxEngine;
pub use engine::SyntaxEngine;
pub use policy::{RetentionPolicy, SyntaxHotness, SyntaxTier, TierCfg, TieredSyntaxPolicy};
use scheduling::CompletedSyntaxTask;
pub(crate) use scheduling::DocSched;
pub(crate) use tasks::TaskCollector;
use tasks::{TaskKind, TaskSpec};
pub(crate) use types::PendingIncrementalEdits;
pub use types::{
	DocEpoch, EditSource, EnsureSyntaxContext, OptKey, SyntaxPollOutcome, SyntaxPollResult,
	SyntaxSlot, TaskId,
};
#[cfg(test)]
pub(crate) use xeno_runtime_language::LanguageId;

const DEFAULT_MAX_CONCURRENCY: usize = 2;
const VIEWPORT_LOOKBEHIND: u32 = 8192;
const VIEWPORT_LOOKAHEAD: u32 = 8192;
const VIEWPORT_WINDOW_MAX: u32 = 128 * 1024;

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
	/// Tiered policy mapping file size to specific configurations.
	policy: TieredSyntaxPolicy,
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
		Self::new(DEFAULT_MAX_CONCURRENCY)
	}
}

impl SyntaxManager {
	pub fn new(max_concurrency: usize) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
			permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
			entries: HashMap::new(),
			engine: Arc::new(RealSyntaxEngine),
			collector: TaskCollector::new(),
		}
	}

	#[cfg(any(test, doc))]
	pub fn new_with_engine(max_concurrency: usize, engine: Arc<dyn SyntaxEngine>) -> Self {
		Self {
			policy: TieredSyntaxPolicy::test_default(),
			permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
			entries: HashMap::new(),
			engine,
			collector: TaskCollector::new(),
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
	pub fn syntax_doc_version(&self, doc_id: DocumentId) -> Option<u64> {
		self.entries.get(&doc_id)?.slot.tree_doc_version
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
			entry.slot.current = None;
			entry.slot.tree_doc_version = None;
			entry.slot.coverage = None;
			entry.slot.sync_bootstrap_attempted = false;
			mark_updated(&mut entry.slot);
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
			entry.slot.current = None;
			entry.slot.tree_doc_version = None;
			entry.slot.coverage = None;
			entry.slot.pending_incremental = None;
			mark_updated(&mut entry.slot);
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
			mark_updated(&mut entry.slot);
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
					is_viewport: res.is_viewport,
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
					entry.slot.current = None;
					entry.slot.tree_doc_version = None;
					entry.slot.coverage = None;
					entry.slot.sync_bootstrap_attempted = false;
					mark_updated(&mut entry.slot);
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
				entry.slot.coverage = None;
				entry.slot.sync_bootstrap_attempted = false;
				mark_updated(&mut entry.slot);
				updated = true;
				entry.slot.pending_incremental = None;
			}
			entry.slot.last_opts_key = Some(current_opts_key);

			if entry.sched.active_task.is_some() {
				entry.sched.active_task_detached = work_disabled;
			}

			updated
		};

		// 2. Process completed tasks (from local cache)
		{
			let entry = self.entry_mut(doc_id);
			if let Some(done) = entry.sched.completed.take() {
				match done.result {
					Ok(syntax_tree) => {
						let Some(current_lang) = ctx.language_id else {
							return SyntaxPollOutcome {
								result: SyntaxPollResult::NoLanguage,
								updated: was_updated,
							};
						};

						let lang_ok = done.lang_id == current_lang;
						let opts_ok = done.opts == current_opts_key;
						let version_match = done.doc_version == ctx.doc_version;
						let retain_ok = retention_allows_install(
							now,
							&entry.sched,
							cfg.retention_hidden,
							ctx.hotness,
						);

						let allow_install = if done.is_viewport {
							done.doc_version == ctx.doc_version
						} else {
							should_install_completed_parse(
								done.doc_version,
								entry.slot.tree_doc_version,
								ctx.doc_version,
								entry.slot.dirty,
							)
						};

						if work_disabled {
							tracing::trace!(
								?doc_id,
								"Discarding syntax result because work is disabled (Cold)"
							);
						} else if lang_ok && opts_ok && retain_ok && allow_install {
							let is_viewport = done.is_viewport;
							if is_viewport {
								// Viewport trees are always dirty (trigger full parse follow-up)
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
							mark_updated(&mut entry.slot);
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
								entry.slot.current = None;
								entry.slot.tree_doc_version = None;
								entry.slot.coverage = None;
								entry.slot.pending_incremental = None;
								entry.slot.dirty = false;
								entry.sched.force_no_debounce = false;
								mark_updated(&mut entry.slot);
								was_updated = true;
							}
						}
					}
					Err(xeno_runtime_language::syntax::SyntaxError::Timeout) => {
						entry.sched.cooldown_until = Some(now + cfg.cooldown_on_timeout);
						return SyntaxPollOutcome {
							result: SyntaxPollResult::CoolingDown,
							updated: was_updated,
						};
					}
					Err(e) => {
						tracing::warn!(?doc_id, ?tier, error=%e, "Background syntax parse failed");
						entry.sched.cooldown_until = Some(now + cfg.cooldown_on_error);
						return SyntaxPollOutcome {
							result: SyntaxPollResult::CoolingDown,
							updated: was_updated,
						};
					}
				}
			}

			if entry.sched.active_task.is_some() {
				if apply_retention(
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
				} else if work_disabled {
					// Fall through to gating check
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
					entry.slot.current = None;
					entry.slot.tree_doc_version = None;
					entry.slot.coverage = None;
					mark_updated(&mut entry.slot);
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

			if apply_retention(
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

		if let Some(res) = sync_result {
			match res {
				Ok(syntax) => {
					let entry = self.entry_mut(doc_id);
					// Re-check invariants after dropping and re-acquiring borrow
					let is_bootstrap = entry.slot.current.is_none();
					let is_visible = matches!(ctx.hotness, SyntaxHotness::Visible);
					if entry.sched.epoch == pre_epoch
						&& is_bootstrap && is_visible
						&& entry.sched.active_task.is_none()
					{
						entry.slot.current = Some(syntax);
						entry.slot.language_id = Some(lang_id);
						entry.slot.tree_doc_version = Some(ctx.doc_version);
						entry.slot.dirty = false;
						entry.slot.coverage = None;
						entry.slot.pending_incremental = None;
						entry.sched.force_no_debounce = false;
						entry.sched.cooldown_until = None;
						mark_updated(&mut entry.slot);
						return SyntaxPollOutcome {
							result: SyntaxPollResult::Ready,
							updated: true,
						};
					}
				}
				Err(_) => {
					// Sync attempt timed out or failed.
					// Fall through to schedule background task.
				}
			}
		}

		// 6. Schedule new task
		// 6a. Kicking ViewportParse for L-tier files
		{
			let entry = self.entry_mut(doc_id);

			let needs_viewport = if tier == SyntaxTier::L
				&& ctx.hotness == SyntaxHotness::Visible
				&& let Some(viewport) = &ctx.viewport
			{
				if entry.slot.current.is_none() {
					true
				} else if entry.slot.current.as_ref().is_some_and(|s| s.is_partial()) {
					// Check coverage: re-kick if viewport moved outside sealed window
					if let Some(coverage) = &entry.slot.coverage {
						viewport.start < coverage.start || viewport.end > coverage.end
					} else {
						true
					}
				} else {
					false
				}
			} else {
				false
			};

			if needs_viewport
				&& entry.sched.active_task.is_none()
				&& let Some(viewport) = &ctx.viewport
			{
				// Drop existing partial tree if we're re-kicking due to move
				if entry.slot.current.as_ref().is_some_and(|s| s.is_partial()) {
					entry.slot.current = None;
					entry.slot.tree_doc_version = None;
					entry.slot.coverage = None;
					mark_updated(&mut entry.slot);
				}

				let win_start = viewport.start.saturating_sub(VIEWPORT_LOOKBEHIND);
				let mut win_end = (viewport.end + VIEWPORT_LOOKAHEAD).min(bytes as u32);
				let mut win_len = win_end.saturating_sub(win_start);

				if win_len > VIEWPORT_WINDOW_MAX {
					// Clamp to max size, biasing towards viewport start
					win_len = VIEWPORT_WINDOW_MAX;
					win_end = (win_start + win_len).min(bytes as u32);
				}

				if win_len > 0 {
					let spec = TaskSpec {
						doc_id,
						epoch: entry.sched.epoch,
						doc_version: ctx.doc_version,
						lang_id,
						opts_key: current_opts_key,
						opts: SyntaxOptions {
							parse_timeout: Duration::from_millis(15), // Very short for viewport
							injections: InjectionPolicy::Disabled,    // No injections in viewport-first
						},
						kind: TaskKind::ViewportParse {
							content: ctx.content.clone(),
							window: win_start..win_end,
						},
						loader: Arc::clone(ctx.loader),
					};

					let permits = Arc::clone(&self.permits);
					let engine = Arc::clone(&self.engine);

					if let Some(task_id) = self.collector.spawn(permits, engine, spec) {
						let entry = self.entries.get_mut(&doc_id).unwrap();
						entry.sched.active_task = Some(task_id);
						entry.sched.force_no_debounce = false;
						return SyntaxPollOutcome {
							result: SyntaxPollResult::Kicked,
							updated: was_updated,
						};
					}
				}
			}
		}

		let (kind, epoch) = {
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
			(kind, entry.sched.epoch)
		};

		let spec = TaskSpec {
			doc_id,
			epoch,
			doc_version: ctx.doc_version,
			lang_id,
			opts_key: current_opts_key,
			opts: SyntaxOptions {
				parse_timeout: cfg.parse_timeout,
				injections: cfg.injections,
			},
			kind,
			loader: Arc::clone(ctx.loader),
		};

		let permits = Arc::clone(&self.permits);
		let engine = Arc::clone(&self.engine);

		if let Some(task_id) = self.collector.spawn(permits, engine, spec) {
			let entry = self.entry_mut(doc_id);
			entry.sched.active_task = Some(task_id);
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
}

/// Invariant enforcement: Checks if a completed background parse should be installed into a slot.
pub(crate) fn should_install_completed_parse(
	done_version: u64,
	current_tree_version: Option<u64>,
	target_version: u64,
	slot_dirty: bool,
) -> bool {
	if let Some(v) = current_tree_version
		&& done_version < v
	{
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
				state.current = None;
				state.tree_doc_version = None;
				state.coverage = None;
				state.sync_bootstrap_attempted = false;
				state.dirty = false;
				state.pending_incremental = None;
				mark_updated(state);
				true
			} else {
				false
			}
		}
		RetentionPolicy::DropAfter(ttl) => {
			if (state.current.is_some() || state.dirty)
				&& now.duration_since(st.last_visible_at) > ttl
			{
				state.current = None;
				state.tree_doc_version = None;
				state.coverage = None;
				state.sync_bootstrap_attempted = false;
				state.dirty = false;
				state.pending_incremental = None;
				mark_updated(state);
				true
			} else {
				false
			}
		}
	}
}

#[cfg(test)]
mod invariants;

#[cfg(test)]
mod tests;
