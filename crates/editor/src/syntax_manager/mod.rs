//! Syntax highlighting with background parsing and tiered policy.
//!
//! # Purpose
//!
//! - Owns: Background parsing scheduling, tiered syntax policy, and grammar loading.
//! - Does not own: Rendering (owned by buffer render logic), tree-sitter core (external).
//! - Source of truth: [`SyntaxManager`].
//!
//! # Mental model
//!
//! - Terms: Tier (S/M/L policy), Hotness (Visible/Warm/Cold), Inflight (background task),
//!   Cooldown (backoff after error).
//! - Lifecycle in one sentence: Edits trigger a debounced background parse, which installs results even if stale to ensure continuous highlighting.
//!
//! # Key types
//!
//! | Type | Meaning | Constraints | Constructed / mutated in |
//! |---|---|---|---|
//! | [`SyntaxManager`] | Top-level scheduler | Global concurrency limit | `EditorState` |
//! | [`SyntaxHotness`] | Visibility / priority | Affects retention/parsing | Render loop / pipeline |
//! | [`SyntaxTier`] | Size-based config (S/M/L) | Controls timeouts/injections | [`SyntaxManager::ensure_syntax`] |
//!
//! # Invariants
//!
//! 1. MUST NOT block UI thread on parsing.
//!    - Enforced in: [`SyntaxManager::ensure_syntax`] (uses `spawn_blocking`)
//!    - Tested by: `syntax_manager::tests::test_inflight_drained_even_if_doc_marked_clean`
//!    - Failure symptom: UI freezes or jitters during edits.
//!
//! 2. MUST enforce single-flight per document.
//!    - Enforced in: `DocState::inflight` check in [`SyntaxManager::ensure_syntax`].
//!    - Tested by: `syntax_manager::tests::test_single_flight_per_doc`
//!    - Failure symptom: Multiple redundant parse tasks for the same document identity.
//!
//! 3. MUST install last completed parse even if stale, but MUST NOT overwrite a newer clean tree.
//!    - Enforced in: [`should_install_completed_parse`] (called from [`SyntaxManager::ensure_syntax`] poll inflight branch).
//!    - Tested by: `syntax_manager::tests::test_stale_parse_does_not_overwrite_clean_incremental`, `syntax_manager::tests::test_stale_install_continuity`
//!    - Failure symptom (missing install): Document stays unhighlighted until an exact match completes.
//!    - Failure symptom (overwrite race): Stale tree overwrites correct incremental tree while `dirty=false`, creating a stuck state with wrong highlights.
//!    - Notes: Stale installs are allowed when the caller is already dirty (catch-up mode) or has no syntax at all (bootstrap). A clean tree from a successful incremental update MUST NOT be replaced by an older full-parse result.
//!
//! 4. MUST call [`SyntaxManager::note_edit_incremental`] (or [`SyntaxManager::note_edit`]) on every document mutation (edits, undo, redo, LSP workspace edits).
//!    - Enforced in: `EditorUndoHost::apply_transaction_inner`, `EditorUndoHost::apply_history_op`, `Editor::apply_buffer_edit_plan`
//!    - Tested by: `syntax_manager::tests::test_note_edit_updates_timestamp`
//!    - Failure symptom: Debounce gate in [`SyntaxManager::ensure_syntax`] is non-functional; background parses fire without waiting for edit silence.
//!
//! 5. MUST skip debounce for bootstrap parses (no existing syntax tree).
//!    - Enforced in: [`SyntaxManager::ensure_syntax`] (debounce gate conditioned on `state.current.is_some()`)
//!    - Tested by: `syntax_manager::tests::test_bootstrap_parse_skips_debounce`
//!    - Failure symptom: Newly opened documents show unhighlighted text until the debounce timeout elapses.
//!
//! 6. MUST detect completed inflight syntax tasks from `tick()`, not only from `render()`.
//!    - Enforced in: `Editor::tick` (calls [`SyntaxManager::any_task_finished`] to trigger redraw)
//!    - Tested by: `syntax_manager::tests::test_idle_tick_polls_inflight_parse`
//!    - Failure symptom: Completed background parses are not installed until user input triggers a render; documents stay unhighlighted indefinitely while idle.
//!
//! 7. MUST bump `syntax_version` whenever the installed tree changes or is dropped.
//!    - Enforced in: `mark_updated` (called from `SyntaxManager::ensure_syntax`, `apply_retention`)
//!    - Tested by: `syntax_manager::tests::test_syntax_version_bumps_on_install`
//!    - Failure symptom: Highlight cache serves stale spans after a reparse or retention drop.
//!
//! 8. MUST clear `pending_incremental` on language change, syntax reset, and retention drop.
//!    - Enforced in: [`SyntaxManager::ensure_syntax`] (language change), [`SyntaxManager::reset_syntax`], `apply_retention`
//!    - Tested by: `syntax_manager::tests::test_language_switch_discards_old_parse`
//!    - Failure symptom: Stale changeset applied against a mismatched rope causes incorrect `InputEdit`s and garbled highlights or panics.
//!
//! # Data flow
//!
//! ## Full reparse (bootstrap or no accumulated edits)
//!
//! 1. Trigger: [`SyntaxManager::note_edit`] called from edit paths to record debounce timestamp.
//! 2. Tick loop: `Editor::tick` checks [`SyntaxManager::any_task_finished`] every iteration and requests a redraw when a background parse completes, ensuring results are installed even when the render loop is idle.
//! 3. Render loop: `Editor::render` calls `ensure_syntax_for_buffers` to kick new parses and install completed results before drawing.
//! 4. Gating: Check visibility, size tier, debounce, and cooldown.
//! 5. Throttling: Acquire global concurrency permit (semaphore).
//! 6. Async boundary: `spawn_blocking` calls [`SyntaxEngine::parse`] (`Syntax::new`).
//! 7. Install: Polled result is installed; `dirty` flag cleared only if versions match.
//!
//! ## Synchronous incremental update (primary path for interactive edits)
//!
//! 1. Trigger: [`SyntaxManager::note_edit_incremental`] called with old/new rope, changeset,
//!    and loader. Changesets are composed via [`ChangeSet::compose`] into
//!    [`PendingIncrementalEdits`].
//! 2. The same call applies [`Syntax::update_from_changeset`] in-line with a 10 ms timeout.
//!    On success the tree is immediately up-to-date, the dirty flag is cleared, and no
//!    background reparse is needed.
//! 3. On failure (timeout or error): state is left dirty with accumulated changesets; the
//!    background path picks up after debounce.
//!
//! ## Background incremental reparse (fallback for sync timeout or large edits)
//!
//! 1. [`SyntaxManager::ensure_syntax`] detects a dirty doc with [`PendingIncrementalEdits`].
//! 2-5. Same gating, throttling, and scheduling as full reparse.
//! 6. Async boundary: `spawn_blocking` clones the existing `Syntax` + composed changeset,
//!    calls [`SyntaxEngine::update_incremental`] (`Syntax::update_from_changeset`).
//!    Falls back to full reparse on failure. The original tree stays in `state.current`
//!    for rendering during the reparse window (no highlight flash).
//! 7. Install: Same as full reparse.
//!
//! # Lifecycle
//!
//! - Idle: Document is clean or cooling down.
//! - Debouncing: Waiting for edit silence.
//! - In-flight: Background task running.
//! - Ready: Syntax installed and version matches.
//!
//! # Concurrency and ordering
//!
//! - Bounded Concurrency: Max N (default 2) global parse tasks via semaphore.
//! - Install Discipline: Results only clear `dirty` if `parse_version == current_version`.
//!
//! # Failure modes and recovery
//!
//! - Parse Timeout: Set cooldown timer; retry after backoff.
//! - Grammar Missing: Return `JitDisabled` error; stop retrying for that session.
//! - Stale Results: Installed to maintain some highlighting, but `dirty` flag triggers eventual catch-up.
//!
//! # Recipes
//!
//! ## Change tier thresholds
//!
//! 1. Update [`TieredSyntaxPolicy::default()`].
//! 2. Ensure `max_bytes_inclusive` logic in [`TieredSyntaxPolicy::tier_for_bytes`] matches.
//!
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ropey::Rope;
use rustc_hash::FxHashSet;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use xeno_primitives::ChangeSet;
use xeno_runtime_language::syntax::{InjectionPolicy, Syntax, SyntaxError, SyntaxOptions};
use xeno_runtime_language::{LanguageId, LanguageLoader};

use crate::buffer::DocumentId;

const DEFAULT_MAX_CONCURRENCY: usize = 2;

/// Parsing visibility / urgency.
///
/// The scheduler uses this to decide whether to keep trees around and whether
/// to run parses at all when the doc isn't currently rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxHotness {
	/// Actively displayed (we need highlights now).
	Visible,
	/// Not visible but likely to become visible soon (e.g. split/tab MRU).
	Warm,
	/// Not visible; safe to drop heavy state.
	Cold,
}

/// File-size tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxTier {
	S,
	M,
	L,
}

/// Tier configuration.
#[derive(Debug, Clone, Copy)]
pub struct TierCfg {
	pub max_bytes_inclusive: Option<usize>,
	pub parse_timeout: Duration,
	pub debounce: Duration,
	pub cooldown_on_timeout: Duration,
	pub cooldown_on_error: Duration,
	pub injections: InjectionPolicy,
	pub retention_hidden: RetentionPolicy,
	pub parse_when_hidden: bool,
}

/// Syntax tree retention policy (memory control).
#[derive(Debug, Clone, Copy)]
pub enum RetentionPolicy {
	/// Never drop.
	Keep,
	/// Drop immediately once hidden (or cold).
	DropWhenHidden,
	/// Drop after a TTL since last Visible.
	DropAfter(Duration),
}

/// Tiered policy: compute tier from size -> cfg.
#[derive(Debug, Clone)]
pub struct TieredSyntaxPolicy {
	s: TierCfg,
	m: TierCfg,
	l: TierCfg,
}

impl Default for TieredSyntaxPolicy {
	fn default() -> Self {
		Self {
			s: TierCfg {
				max_bytes_inclusive: Some(256 * 1024),
				parse_timeout: Duration::from_millis(500),
				debounce: Duration::from_millis(80),
				cooldown_on_timeout: Duration::from_millis(400),
				cooldown_on_error: Duration::from_millis(150),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::Keep,
				parse_when_hidden: false,
			},
			m: TierCfg {
				max_bytes_inclusive: Some(1024 * 1024),
				parse_timeout: Duration::from_millis(1200),
				debounce: Duration::from_millis(140),
				cooldown_on_timeout: Duration::from_secs(2),
				cooldown_on_error: Duration::from_millis(250),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::DropAfter(Duration::from_secs(60)),
				parse_when_hidden: false,
			},
			l: TierCfg {
				max_bytes_inclusive: None,
				parse_timeout: Duration::from_secs(3),
				debounce: Duration::from_millis(250),
				cooldown_on_timeout: Duration::from_secs(10),
				cooldown_on_error: Duration::from_secs(2),
				injections: InjectionPolicy::Disabled, // biggest win: avoid injection layer explosion
				retention_hidden: RetentionPolicy::DropWhenHidden,
				parse_when_hidden: false,
			},
		}
	}
}

impl TieredSyntaxPolicy {
	pub fn tier_for_bytes(&self, bytes: usize) -> SyntaxTier {
		if bytes <= self.s.max_bytes_inclusive.unwrap() {
			SyntaxTier::S
		} else if bytes <= self.m.max_bytes_inclusive.unwrap() {
			SyntaxTier::M
		} else {
			SyntaxTier::L
		}
	}

	pub fn cfg(&self, tier: SyntaxTier) -> TierCfg {
		match tier {
			SyntaxTier::S => self.s,
			SyntaxTier::M => self.m,
			SyntaxTier::L => self.l,
		}
	}
}

/// Key for checking if parse options have changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OptKey {
	injections: InjectionPolicy,
}

struct DocState {
	last_edit_at: Instant,
	last_visible_at: Instant,
	cooldown_until: Option<Instant>,
	inflight: Option<PendingSyntaxTask>,
}

impl DocState {
	fn new(now: Instant) -> Self {
		Self {
			last_edit_at: now,
			last_visible_at: now,
			cooldown_until: None,
			inflight: None,
		}
	}
}

struct PendingSyntaxTask {
	doc_version: u64,
	lang_id: LanguageId,
	opts: OptKey,
	_started_at: Instant,
	task: JoinHandle<Result<Syntax, SyntaxError>>,
}

/// Result of polling syntax state.
#[derive(Debug, PartialEq, Eq)]
pub enum SyntaxPollResult {
	/// Syntax is ready.
	Ready,
	/// Parse is pending in background.
	Pending,
	/// Parse was kicked off.
	Kicked,
	/// No language configured for this document.
	NoLanguage,
	/// Cooldown active after timeout/error.
	CoolingDown,
	/// Background parsing disabled for this state (e.g. hidden large file).
	Disabled,
	/// Throttled by global concurrency cap.
	Throttled,
}

/// Abstract engine for parsing syntax (for test mockability).
pub trait SyntaxEngine: Send + Sync {
	fn parse(
		&self,
		content: ropey::RopeSlice<'_>,
		lang: LanguageId,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError>;

	/// Incrementally updates an existing syntax tree via a composed changeset.
	///
	/// The default implementation discards the old tree and falls back to a
	/// full reparse, allowing mock engines to remain simple.
	fn update_incremental(
		&self,
		_syntax: Syntax,
		_old_source: ropey::RopeSlice<'_>,
		new_source: ropey::RopeSlice<'_>,
		_changeset: &ChangeSet,
		lang: LanguageId,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError> {
		self.parse(new_source, lang, loader, opts)
	}
}

struct RealSyntaxEngine;
impl SyntaxEngine for RealSyntaxEngine {
	fn parse(
		&self,
		content: ropey::RopeSlice<'_>,
		lang: LanguageId,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError> {
		Syntax::new(content, lang, loader, opts)
	}

	fn update_incremental(
		&self,
		mut syntax: Syntax,
		old_source: ropey::RopeSlice<'_>,
		new_source: ropey::RopeSlice<'_>,
		changeset: &ChangeSet,
		lang: LanguageId,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError> {
		syntax
			.update_from_changeset(old_source, new_source, changeset, loader, opts)
			.map(|()| syntax)
			.or_else(|e| {
				tracing::warn!(error = %e, "Incremental parse failed, falling back to full reparse");
				Syntax::new(new_source, lang, loader, opts)
			})
	}
}

/// Background syntax scheduling + parsing.
pub struct SyntaxManager {
	policy: TieredSyntaxPolicy,
	permits: Arc<Semaphore>,
	docs: HashMap<DocumentId, DocState>,
	syntax: HashMap<DocumentId, SyntaxState>,
	engine: Arc<dyn SyntaxEngine>,
	dirty_docs: FxHashSet<DocumentId>,
}

impl Default for SyntaxManager {
	fn default() -> Self {
		Self::new(DEFAULT_MAX_CONCURRENCY)
	}
}

pub struct EnsureSyntaxContext<'a> {
	pub doc_id: DocumentId,
	pub doc_version: u64,
	pub language_id: Option<LanguageId>,
	pub content: &'a Rope,
	pub hotness: SyntaxHotness,
	pub loader: &'a Arc<LanguageLoader>,
}

/// Accumulated edits awaiting an incremental reparse.
///
/// Between [`SyntaxManager::note_edit_incremental`] calls and the next
/// background parse, individual changesets are composed into a single
/// changeset relative to the rope snapshot taken at the first edit.
struct PendingIncrementalEdits {
	old_rope: Rope,
	composed: ChangeSet,
}

#[derive(Default)]
pub struct SyntaxState {
	current: Option<Syntax>,
	dirty: bool,
	updated: bool,
	version: u64,
	language_id: Option<LanguageId>,
	pending_incremental: Option<PendingIncrementalEdits>,
}

pub struct SyntaxPollOutcome {
	pub result: SyntaxPollResult,
	pub updated: bool,
}

impl SyntaxManager {
	pub fn new(max_concurrency: usize) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
			permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
			docs: HashMap::new(),
			syntax: HashMap::new(),
			engine: Arc::new(RealSyntaxEngine),
			dirty_docs: FxHashSet::default(),
		}
	}

	#[cfg(test)]
	pub fn new_with_engine(max_concurrency: usize, engine: Arc<dyn SyntaxEngine>) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
			permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
			docs: HashMap::new(),
			syntax: HashMap::new(),
			engine,
			dirty_docs: FxHashSet::default(),
		}
	}

	pub fn set_policy(&mut self, policy: TieredSyntaxPolicy) {
		self.policy = policy;
	}

	fn state_mut(&mut self, doc_id: DocumentId) -> &mut SyntaxState {
		self.syntax.entry(doc_id).or_default()
	}

	pub fn has_syntax(&self, doc_id: DocumentId) -> bool {
		self.syntax
			.get(&doc_id)
			.and_then(|state| state.current.as_ref())
			.is_some()
	}

	pub fn is_dirty(&self, doc_id: DocumentId) -> bool {
		self.syntax
			.get(&doc_id)
			.map(|state| state.dirty)
			.unwrap_or(false)
	}

	pub fn syntax_for_doc(&self, doc_id: DocumentId) -> Option<&Syntax> {
		self.syntax
			.get(&doc_id)
			.and_then(|state| state.current.as_ref())
	}

	pub fn syntax_version(&self, doc_id: DocumentId) -> u64 {
		self.syntax
			.get(&doc_id)
			.map(|state| state.version)
			.unwrap_or(0)
	}

	pub fn reset_syntax(&mut self, doc_id: DocumentId) {
		let state = self.state_mut(doc_id);
		if state.current.is_some() {
			state.current = None;
			mark_updated(state);
		}
		state.dirty = true;
		state.pending_incremental = None;
		self.dirty_docs.insert(doc_id);
	}

	pub fn mark_dirty(&mut self, doc_id: DocumentId) {
		self.state_mut(doc_id).dirty = true;
		self.dirty_docs.insert(doc_id);
	}

	/// Records an edit for debounce scheduling without changeset data.
	///
	/// Inflight tasks are intentionally left running (single-flight discipline).
	pub fn note_edit(&mut self, doc_id: DocumentId) {
		let now = Instant::now();
		self.docs
			.entry(doc_id)
			.or_insert_with(|| DocState::new(now))
			.last_edit_at = now;
		self.mark_dirty(doc_id);
	}

	/// Records an edit and applies an incremental tree-sitter update.
	///
	/// Combines three steps into one call:
	///
	/// 1. Updates debounce timestamp and marks the document dirty (same as
	///    [`note_edit`](Self::note_edit)).
	/// 2. Accumulates the changeset into [`PendingIncrementalEdits`] so the
	///    background path can use [`Syntax::update_from_changeset`] as a
	///    fallback.
	/// 3. Attempts a synchronous incremental reparse (10 ms timeout). On
	///    success the tree is immediately up-to-date and the dirty flag is
	///    cleared, eliminating the need for a background reparse.
	///
	/// If no existing syntax tree is present (bootstrap), the changeset is
	/// discarded and the method behaves identically to [`note_edit`](Self::note_edit).
	pub fn note_edit_incremental(
		&mut self,
		doc_id: DocumentId,
		old_rope: &Rope,
		new_rope: &Rope,
		changeset: &ChangeSet,
		loader: &LanguageLoader,
	) {
		const SYNC_TIMEOUT: Duration = Duration::from_millis(10);

		let now = Instant::now();
		self.docs
			.entry(doc_id)
			.or_insert_with(|| DocState::new(now))
			.last_edit_at = now;
		self.mark_dirty(doc_id);

		let state = self.syntax.entry(doc_id).or_default();
		if state.current.is_none() {
			return;
		}

		match state.pending_incremental.take() {
			Some(mut pending) => {
				pending.composed = pending.composed.compose(changeset.clone());
				state.pending_incremental = Some(pending);
			}
			None => {
				state.pending_incremental = Some(PendingIncrementalEdits {
					old_rope: old_rope.clone(),
					composed: changeset.clone(),
				});
			}
		}

		let syntax = state.current.as_mut().expect("checked above");
		let opts = SyntaxOptions {
			parse_timeout: SYNC_TIMEOUT,
			..syntax.opts()
		};

		match syntax.update_from_changeset(
			old_rope.slice(..),
			new_rope.slice(..),
			changeset,
			loader,
			opts,
		) {
			Ok(()) => {
				state.pending_incremental = None;
				state.dirty = false;
				self.dirty_docs.remove(&doc_id);
				mark_updated(state);
			}
			Err(e) => {
				tracing::debug!(error = %e, ?doc_id, "Sync incremental update failed");
			}
		}
	}

	/// Cleans up tracking state for a closed document.
	pub fn on_document_close(&mut self, doc_id: DocumentId) {
		self.forget_doc(doc_id);
	}

	/// Removes all tracking state and pending tasks for a document.
	pub fn forget_doc(&mut self, doc_id: DocumentId) {
		self.syntax.remove(&doc_id);
		self.dirty_docs.remove(&doc_id);
		if let Some(mut st) = self.docs.remove(&doc_id)
			&& let Some(p) = st.inflight.take()
		{
			p.task.abort();
		}
	}

	pub fn has_pending(&self, doc_id: DocumentId) -> bool {
		self.docs
			.get(&doc_id)
			.and_then(|d| d.inflight.as_ref())
			.is_some()
	}

	pub fn pending_count(&self) -> usize {
		self.docs.values().filter(|d| d.inflight.is_some()).count()
	}

	pub fn pending_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.docs
			.iter()
			.filter(|(_, d)| d.inflight.is_some())
			.map(|(id, _)| *id)
	}

	pub fn dirty_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.dirty_docs.iter().copied()
	}

	/// Returns true if any inflight task has finished.
	///
	/// Uses `JoinHandle::is_finished()` for a non-consuming, zero-cost check.
	/// The caller should trigger a redraw so that `ensure_syntax` can poll and
	/// install the result.
	pub fn any_task_finished(&self) -> bool {
		self.docs
			.values()
			.any(|d| d.inflight.as_ref().is_some_and(|t| t.task.is_finished()))
	}

	/// Polls or kicks background syntax parsing.
	///
	/// This is the main entry point for the syntax scheduler. It:
	/// 1. Polling/draining inflight tasks (immortal task prevention).
	/// 2. Validating results against LanguageId and OptKey.
	/// 3. Respecting retention policy at completion time.
	/// 4. Handling debounce and backoff (cooldown) timers.
	/// 5. Spawning new background parse tasks if permits are available.
	pub fn ensure_syntax(&mut self, ctx: EnsureSyntaxContext<'_>) -> SyntaxPollOutcome {
		let now = Instant::now();
		let docs = &mut self.docs;
		let syntax = &mut self.syntax;

		let st = docs.entry(ctx.doc_id).or_insert_with(|| DocState::new(now));
		let state = syntax.entry(ctx.doc_id).or_default();
		state.updated = false;

		if state.language_id != ctx.language_id {
			if let Some(pending) = st.inflight.take() {
				pending.task.abort();
			}
			if state.current.is_some() {
				state.current = None;
				mark_updated(state);
			}
			state.dirty = true;
			state.pending_incremental = None;
			self.dirty_docs.insert(ctx.doc_id);
			state.language_id = ctx.language_id;
		}

		if matches!(ctx.hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
			st.last_visible_at = now;
		}

		let bytes = ctx.content.len_bytes();
		let tier = self.policy.tier_for_bytes(bytes);
		let cfg = self.policy.cfg(tier);
		let current_opts_key = OptKey {
			injections: cfg.injections,
		};

		// 1) Poll/drain inflight FIRST (fixes “immortal inflight”)
		if let Some(p) = st.inflight.as_mut() {
			let join = xeno_primitives::future::poll_once(&mut p.task);
			if join.is_none() {
				// still running; do NOT short-circuit to Ready/Disabled
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Pending,
					updated: state.updated,
				};
			}

			let done = st.inflight.take().expect("inflight present");
			match join.expect("checked ready") {
				Ok(Ok(syntax_tree)) => {
					// language gating: only install if language still matches
					let Some(current_lang) = ctx.language_id else {
						// language removed mid-flight: discard result
						return SyntaxPollOutcome {
							result: SyntaxPollResult::NoLanguage,
							updated: state.updated,
						};
					};

					let lang_ok = done.lang_id == current_lang;
					let opts_ok = done.opts == current_opts_key;
					let version_match = done.doc_version == ctx.doc_version;
					let retain_ok =
						retention_allows_install(now, st, cfg.retention_hidden, ctx.hotness);

					let allow_install = should_install_completed_parse(
						version_match,
						state.dirty,
						state.current.is_some(),
					);

					if lang_ok && retain_ok && allow_install {
						state.current = Some(syntax_tree);
						state.language_id = Some(current_lang);
						mark_updated(state);
					}

					// dirty clears only if this parse matches current version + "shape"
					if lang_ok && opts_ok && version_match {
						state.dirty = false;
						self.dirty_docs.remove(&ctx.doc_id);
						st.cooldown_until = None;
						return SyntaxPollOutcome {
							result: SyntaxPollResult::Ready,
							updated: state.updated,
						};
					}
					// else: keep dirty true (caller keeps chasing newest)
				}
				Ok(Err(SyntaxError::Timeout)) => {
					st.cooldown_until = Some(now + cfg.cooldown_on_timeout);
					return SyntaxPollOutcome {
						result: SyntaxPollResult::CoolingDown,
						updated: state.updated,
					};
				}
				Ok(Err(e)) => {
					tracing::warn!(doc_id=?ctx.doc_id, tier=?tier, error=%e, "Background syntax parse failed");
					st.cooldown_until = Some(now + cfg.cooldown_on_error);
					return SyntaxPollOutcome {
						result: SyntaxPollResult::CoolingDown,
						updated: state.updated,
					};
				}
				Err(e) => {
					tracing::warn!(doc_id=?ctx.doc_id, tier=?tier, error=%e, "Background syntax task panicked");
					st.cooldown_until = Some(now + cfg.cooldown_on_error);
					return SyntaxPollOutcome {
						result: SyntaxPollResult::CoolingDown,
						updated: state.updated,
					};
				}
			}
		}

		// 2) Handle “no language” AFTER draining inflight to avoid leaks
		let Some(lang_id) = ctx.language_id else {
			if state.current.is_some() {
				state.current = None;
				mark_updated(state);
			}
			state.language_id = None;
			state.dirty = false;
			self.dirty_docs.remove(&ctx.doc_id);
			st.cooldown_until = None;
			return SyntaxPollOutcome {
				result: SyntaxPollResult::NoLanguage,
				updated: state.updated,
			};
		};

		// 3) Retention AFTER inflight completion (don’t re-install dropped trees)
		apply_retention(
			now,
			st,
			cfg.retention_hidden,
			ctx.hotness,
			state,
			ctx.doc_id,
			&mut self.dirty_docs,
		);

		// 4) Clean short-circuit (safe now: no inflight exists)
		if state.current.is_some() && !state.dirty {
			return SyntaxPollOutcome {
				result: SyntaxPollResult::Ready,
				updated: state.updated,
			};
		}

		// 5) Hidden policy check (safe now: inflight already drained)
		if !matches!(ctx.hotness, SyntaxHotness::Visible) && !cfg.parse_when_hidden {
			return SyntaxPollOutcome {
				result: SyntaxPollResult::Disabled,
				updated: state.updated,
			};
		}

		// 6) Debounce (skip for bootstrap: no existing tree → parse immediately)
		if state.current.is_some() && now.duration_since(st.last_edit_at) < cfg.debounce {
			return SyntaxPollOutcome {
				result: SyntaxPollResult::Pending,
				updated: state.updated,
			};
		}

		// 7) Cooldown
		if let Some(until) = st.cooldown_until
			&& now < until
		{
			return SyntaxPollOutcome {
				result: SyntaxPollResult::CoolingDown,
				updated: state.updated,
			};
		}

		// 8) Global concurrency cap
		let permit = match self.permits.clone().try_acquire_owned() {
			Ok(p) => p,
			Err(_) => {
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Throttled,
					updated: state.updated,
				};
			}
		};

		let content = ctx.content.clone();
		let loader = Arc::clone(ctx.loader);
		let engine = Arc::clone(&self.engine);

		let opts = SyntaxOptions {
			parse_timeout: cfg.parse_timeout,
			injections: cfg.injections,
			..SyntaxOptions::default()
		};

		let incremental = state.pending_incremental.take().and_then(|pending| {
			state
				.current
				.as_ref()
				.map(|syntax| (syntax.clone(), pending))
		});

		let task = tokio::task::spawn_blocking(move || {
			let _permit: OwnedSemaphorePermit = permit;
			if let Some((syntax, pending)) = incremental {
				engine.update_incremental(
					syntax,
					pending.old_rope.slice(..),
					content.slice(..),
					&pending.composed,
					lang_id,
					&loader,
					opts,
				)
			} else {
				engine.parse(content.slice(..), lang_id, &loader, opts)
			}
		});

		st.inflight = Some(PendingSyntaxTask {
			doc_version: ctx.doc_version,
			lang_id,
			opts: current_opts_key,
			_started_at: now,
			task,
		});

		SyntaxPollOutcome {
			result: SyntaxPollResult::Kicked,
			updated: state.updated,
		}
	}
}

/// Whether a completed background parse should be installed into the syntax slot.
///
/// Guards against overwriting a newer incremental tree with a stale full-parse
/// result. A stale parse (version mismatch) is only installed when the caller is
/// already dirty (catch-up mode) or has no syntax at all (bootstrap).
fn should_install_completed_parse(
	version_match: bool,
	slot_dirty: bool,
	has_current: bool,
) -> bool {
	version_match || slot_dirty || !has_current
}

fn retention_allows_install(
	now: Instant,
	st: &DocState,
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

fn mark_updated(state: &mut SyntaxState) {
	state.updated = true;
	state.version = state.version.wrapping_add(1);
}

fn apply_retention(
	now: Instant,
	st: &DocState,
	policy: RetentionPolicy,
	hotness: SyntaxHotness,
	state: &mut SyntaxState,
	doc_id: DocumentId,
	dirty_docs: &mut FxHashSet<DocumentId>,
) {
	if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
		return;
	}

	match policy {
		RetentionPolicy::Keep => {}
		RetentionPolicy::DropWhenHidden => {
			if state.current.is_some() {
				state.current = None;
				state.dirty = true;
				state.pending_incremental = None;
				dirty_docs.insert(doc_id);
				mark_updated(state);
			}
		}
		RetentionPolicy::DropAfter(ttl) => {
			if state.current.is_some() && now.duration_since(st.last_visible_at) > ttl {
				state.current = None;
				state.dirty = true;
				state.pending_incremental = None;
				dirty_docs.insert(doc_id);
				mark_updated(state);
			}
		}
	}
}

#[cfg(test)]
mod tests;
