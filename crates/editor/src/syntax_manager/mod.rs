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
//! | [`SyntaxSlot`] | Per-document syntax state | Tracks `tree_doc_version` for monotonic install | `SyntaxManager` entry map |
//! | [`lru::RecentDocLru`] | Bounded LRU of recently visible documents | Capacity-limited, O(n) touch | `EditorCore::warm_docs` |
//!
//! # Invariants
//!
//! - MUST NOT perform unbounded parsing on the UI thread.
//!   - Synchronous incremental updates are bounded to 10ms.
//!   - Full parsing and large updates are offloaded to background tasks.
//!   - Enforced in: [`SyntaxManager::ensure_syntax`] (uses `spawn_blocking`) and [`SyntaxManager::note_edit_incremental`] (bounded timeout).
//!   - Tested by: `syntax_manager::tests::test_inflight_drained_even_if_doc_marked_clean`
//!   - Failure symptom: UI freezes or jitters during edits.
//!
//! - MUST enforce single-flight per document.
//!   - Enforced in: `DocEntry::sched.inflight` check in [`SyntaxManager::ensure_syntax`].
//!   - Tested by: `syntax_manager::tests::test_single_flight_per_doc`
//!   - Failure symptom: Multiple redundant parse tasks for the same document identity.
//!
//! - MUST install last completed parse even if stale, but MUST NOT regress to an older tree
//!   version than the one currently installed. Version comparison is monotonic via
//!   `tree_doc_version`.
//!   - Enforced in: [`should_install_completed_parse`] (called from [`SyntaxManager::ensure_syntax`] poll inflight branch).
//!   - Tested by: `syntax_manager::tests::test_stale_parse_does_not_overwrite_clean_incremental`, `syntax_manager::tests::test_stale_install_continuity`
//!   - Failure symptom (missing install): Document stays unhighlighted until an exact match completes.
//!   - Failure symptom (overwrite race): Stale tree overwrites correct incremental tree while `dirty=false`, creating a stuck state with wrong highlights.
//!
//! - MUST call [`SyntaxManager::note_edit_incremental`] (or [`SyntaxManager::note_edit`]) on every document mutation (edits, undo, redo, LSP workspace edits).
//!   - Enforced in: `EditorUndoHost::apply_transaction_inner`, `EditorUndoHost::apply_history_op`, `Editor::apply_buffer_edit_plan`
//!   - Tested by: `syntax_manager::tests::test_note_edit_updates_timestamp`
//!   - Failure symptom: Debounce gate in [`SyntaxManager::ensure_syntax`] is non-functional; background parses fire without waiting for edit silence.
//!
//! - MUST skip debounce for bootstrap parses (no existing syntax tree).
//!   - Enforced in: [`SyntaxManager::ensure_syntax`] (debounce gate conditioned on `state.current.is_some()`)
//!   - Tested by: `syntax_manager::tests::test_bootstrap_parse_skips_debounce`
//!   - Failure symptom: Newly opened documents show unhighlighted text until the debounce timeout elapses.
//!
//! - MUST detect completed inflight syntax tasks from `tick()`, not only from `render()`.
//!   - Enforced in: `Editor::tick` (calls [`SyntaxManager::drain_finished_inflight`] to trigger redraw)
//!   - Tested by: `syntax_manager::tests::test_idle_tick_polls_inflight_parse`
//!   - Failure symptom: Completed background parses are not installed until user input triggers a render; documents stay unhighlighted indefinitely while idle.
//!
//! - MUST bump `syntax_version` whenever the installed tree changes or is dropped.
//!   - Enforced in: `mark_updated` (called from `SyntaxManager::ensure_syntax`, `apply_retention`)
//!   - Tested by: `syntax_manager::tests::test_syntax_version_bumps_on_install`
//!   - Failure symptom: Highlight cache serves stale spans after a reparse or retention drop.
//!
//! - MUST clear `pending_incremental` on language change, syntax reset, and retention drop.
//!   - Enforced in: [`SyntaxManager::ensure_syntax`] (language change), [`SyntaxManager::reset_syntax`], `apply_retention`
//!   - Tested by: `syntax_manager::tests::test_language_switch_discards_old_parse`
//!   - Failure symptom: Stale changeset applied against a mismatched rope causes incorrect `InputEdit`s and garbled highlights or panics.
//!   - Note: On options mismatch, the old tree is retained for rendering continuity while the new parse runs.
//!
//! - MUST track `tree_doc_version` alongside the installed syntax tree; MUST clear it whenever
//!   the tree is dropped (reset, retention, language change).
//!   - Enforced in: `SyntaxManager::note_edit_incremental` (sets on success), `SyntaxManager::ensure_syntax` (sets on install), `SyntaxManager::reset_syntax`, `apply_retention` (clears on drop)
//!   - Tested by: `syntax_manager::tests::test_stale_parse_does_not_overwrite_clean_incremental`
//!   - Failure symptom: Highlight rendering uses a tree from a different document version, causing out-of-bounds access or garbled spans.
//!
//! - Highlight rendering MUST skip spans when `tree_doc_version` does not match the document
//!   version being rendered.
//!   - Enforced in: `HighlightTiles::build_tiles` (bounds check)
//!   - Tested by: TODO (add regression: test_highlight_skips_stale_tree_version)
//!   - Failure symptom: Crash or panic from out-of-bounds tree-sitter node access during rapid edits.
//!
//! - Recently visible documents MUST be promoted to `Warm` hotness via [`lru::RecentDocLru`] to
//!   prevent immediate retention drops when hidden.
//!   - Enforced in: `Editor::ensure_syntax_for_buffers` (touches LRU on visible, checks LRU for warm), `Editor::on_document_close` (removes from LRU)
//!   - Tested by: TODO (add regression: test_warm_hotness_prevents_immediate_drop)
//!   - Failure symptom: Switching away from a buffer for one frame drops its syntax tree, causing a flash of unhighlighted text on return.
//!
//! # Data flow
//!
//! ## Full reparse (bootstrap or no accumulated edits)
//!
//! - Trigger: [`SyntaxManager::note_edit`] called from edit paths to record debounce timestamp.
//! - Tick loop: `Editor::tick` checks [`SyntaxManager::any_task_finished`] every iteration and requests a redraw when a background parse completes, ensuring results are installed even when the render loop is idle.
//! - Render loop: `Editor::render` calls `ensure_syntax_for_buffers` to kick new parses and install completed results before drawing.
//! - Gating: Check visibility, size tier, debounce, and cooldown.
//! - Throttling: Acquire global concurrency permit (semaphore).
//! - Async boundary: `spawn_blocking` calls [`SyntaxEngine::parse`] (`Syntax::new`).
//! - Install: Polled result is installed; `dirty` flag cleared only if versions match.
//!
//! ## Synchronous incremental update (primary path for interactive edits)
//!
//! - Trigger: [`SyntaxManager::note_edit_incremental`] called with old/new rope, changeset,
//!   document version, and loader. Changesets are composed via [`ChangeSet::compose`] into
//!   [`PendingIncrementalEdits`].
//! - The same call applies [`Syntax::update_from_changeset`] in-line with a 10 ms timeout.
//!   On success the tree is immediately up-to-date, `tree_doc_version` is set to the
//!   provided version, the dirty flag is cleared, and no background reparse is needed.
//! - On failure (timeout or error): state is left dirty with accumulated changesets; the
//!   background path picks up after debounce.
//!
//! ## Background incremental reparse (fallback for sync timeout or large edits)
//!
//! - [`SyntaxManager::ensure_syntax`] detects a dirty doc with [`PendingIncrementalEdits`].
//! - Same gating, throttling, and scheduling as full reparse.
//! - Async boundary: `spawn_blocking` clones the existing `Syntax` + composed changeset,
//!   calls [`SyntaxEngine::update_incremental`] (`Syntax::update_from_changeset`).
//!   Falls back to full reparse on failure. The original tree stays in `state.current`
//!   for rendering during the reparse window (no highlight flash).
//! - Install: Same as full reparse.
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
//! - Update [`TieredSyntaxPolicy::default()`].
//! - Ensure `s_max_bytes_inclusive <= m_max_bytes_inclusive`.
//!
use std::collections::HashMap;
pub mod lru;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use xeno_primitives::{ChangeSet, Rope};
use xeno_runtime_language::syntax::{InjectionPolicy, Syntax, SyntaxError, SyntaxOptions};
use xeno_runtime_language::{LanguageId, LanguageLoader};

use crate::core::document::DocumentId;

const DEFAULT_MAX_CONCURRENCY: usize = 2;

/// Visibility and urgency of a document for the syntax scheduler.
///
/// Hotness determines the priority of background parsing tasks and the aggressiveness
/// of syntax tree retention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxHotness {
	/// Actively displayed in a window.
	///
	/// Parsing is high priority and results are always installed.
	Visible,
	/// Not currently visible but likely to become so soon (e.g., recently closed split).
	///
	/// Parsing is allowed but lower priority.
	Warm,
	/// Not visible and not in recent use.
	///
	/// Safe to drop heavy syntax state to save memory.
	Cold,
}

/// Size-based tier for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxTier {
	/// Small file.
	S,
	/// Medium file.
	M,
	/// Large file.
	L,
}

/// Configuration for a specific [`SyntaxTier`].
#[derive(Debug, Clone, Copy)]
pub struct TierCfg {
	/// Maximum time allowed for a single parse operation.
	pub parse_timeout: Duration,
	/// Time to wait after an edit before triggering a background parse.
	pub debounce: Duration,
	/// Backoff duration after a parse timeout.
	pub cooldown_on_timeout: Duration,
	/// Backoff duration after a parse error.
	pub cooldown_on_error: Duration,
	/// Injection handling policy.
	pub injections: InjectionPolicy,
	/// Retention policy for hidden documents.
	pub retention_hidden: RetentionPolicy,
	/// Whether to allow background parsing when the document is not visible.
	pub parse_when_hidden: bool,
}

/// Syntax tree retention policy for memory management.
#[derive(Debug, Clone, Copy)]
pub enum RetentionPolicy {
	/// Never drop the syntax tree.
	Keep,
	/// Drop the syntax tree immediately once the document is hidden.
	DropWhenHidden,
	/// Drop the syntax tree after a TTL since the document was last visible.
	DropAfter(Duration),
}

/// Tiered syntax policy that maps file size to specific configurations.
#[derive(Debug, Clone)]
pub struct TieredSyntaxPolicy {
	/// Threshold for the small (S) tier.
	pub s_max_bytes_inclusive: usize,
	/// Threshold for the medium (M) tier.
	pub m_max_bytes_inclusive: usize,
	/// Configuration for small files.
	pub s: TierCfg,
	/// Configuration for medium files.
	pub m: TierCfg,
	/// Configuration for large files.
	pub l: TierCfg,
}

impl Default for TieredSyntaxPolicy {
	fn default() -> Self {
		Self {
			s_max_bytes_inclusive: 256 * 1024,
			m_max_bytes_inclusive: 1024 * 1024,
			s: TierCfg {
				parse_timeout: Duration::from_millis(500),
				debounce: Duration::from_millis(80),
				cooldown_on_timeout: Duration::from_millis(400),
				cooldown_on_error: Duration::from_millis(150),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::Keep,
				parse_when_hidden: false,
			},
			m: TierCfg {
				parse_timeout: Duration::from_millis(1200),
				debounce: Duration::from_millis(140),
				cooldown_on_timeout: Duration::from_secs(2),
				cooldown_on_error: Duration::from_millis(250),
				injections: InjectionPolicy::Eager,
				retention_hidden: RetentionPolicy::DropAfter(Duration::from_secs(60)),
				parse_when_hidden: false,
			},
			l: TierCfg {
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
		if bytes <= self.s_max_bytes_inclusive {
			SyntaxTier::S
		} else if bytes <= self.m_max_bytes_inclusive {
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

struct DocSched {
	last_edit_at: Instant,
	last_visible_at: Instant,
	cooldown_until: Option<Instant>,
	inflight: Option<PendingSyntaxTask>,
	completed: Option<CompletedSyntaxTask>,
}

impl DocSched {
	fn new(now: Instant) -> Self {
		Self {
			last_edit_at: now,
			last_visible_at: now,
			cooldown_until: None,
			inflight: None,
			completed: None,
		}
	}
}

struct PendingSyntaxTask {
	doc_version: u64,
	lang_id: LanguageId,
	opts: OptKey,
	_started_at: Instant,
	_permit: OwnedSemaphorePermit,
	task: JoinHandle<Result<Syntax, SyntaxError>>,
}

struct CompletedSyntaxTask {
	doc_version: u64,
	lang_id: LanguageId,
	opts: OptKey,
	result: Result<Syntax, SyntaxError>,
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
}

impl Default for SyntaxManager {
	/// Creates a new manager with default concurrency limits.
	fn default() -> Self {
		Self::new(DEFAULT_MAX_CONCURRENCY)
	}
}

/// Context provided to [`SyntaxManager::ensure_syntax`] for scheduling.
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
	/// Document version that `old_rope` corresponds to.
	///
	/// Used to verify that the resident tree still matches the pending base
	/// before attempting an incremental update.
	base_tree_doc_version: u64,
	/// Source text at the start of the pending edit window.
	old_rope: Rope,
	/// Composed delta from `old_rope` to the current document state.
	composed: ChangeSet,
}

/// Per-document syntax state managed by [`SyntaxManager`].
///
/// Tracks the installed syntax tree, its document version, and pending
/// incremental edits. The `tree_doc_version` field enables monotonic
/// version gating to prevent stale parse results from overwriting newer trees.
#[derive(Default)]
pub struct SyntaxSlot {
	/// Currently installed syntax tree, if any.
	current: Option<Syntax>,
	/// Whether the document has been edited since the last successful parse.
	dirty: bool,
	/// Whether the `current` tree was updated in the last poll.
	updated: bool,
	/// Local version counter, bumped whenever `current` changes or is dropped.
	version: u64,
	/// Document version that the `current` syntax tree corresponds to.
	///
	/// Set by `note_edit_incremental` (on sync success) and `ensure_syntax` (on
	/// background install). `None` means no tree is installed.
	tree_doc_version: Option<u64>,
	/// Language identity used for the last parse.
	language_id: Option<LanguageId>,
	/// Accumulated incremental edits awaiting background processing.
	pending_incremental: Option<PendingIncrementalEdits>,
	/// Configuration options used for the last parse.
	last_opts_key: Option<OptKey>,
}

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

pub struct SyntaxPollOutcome {
	pub result: SyntaxPollResult,
	pub updated: bool,
}

impl SyntaxManager {
	pub fn new(max_concurrency: usize) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
			permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
			entries: HashMap::new(),
			engine: Arc::new(RealSyntaxEngine),
		}
	}

	#[cfg(test)]
	pub fn new_with_engine(max_concurrency: usize, engine: Arc<dyn SyntaxEngine>) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
			permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
			entries: HashMap::new(),
			engine,
		}
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

	/// Resets the syntax state for a document, clearing the current tree and history.
	pub fn reset_syntax(&mut self, doc_id: DocumentId) {
		let entry = self.entry_mut(doc_id);
		if entry.slot.current.is_some() {
			entry.slot.current = None;
			entry.slot.tree_doc_version = None;
			mark_updated(&mut entry.slot);
		}
		entry.slot.dirty = true;
		entry.slot.pending_incremental = None;
	}

	/// Marks a document as dirty, triggering a reparse on the next poll.
	pub fn mark_dirty(&mut self, doc_id: DocumentId) {
		self.entry_mut(doc_id).slot.dirty = true;
	}

	/// Records an edit for debounce scheduling without changeset data.
	pub fn note_edit(&mut self, doc_id: DocumentId) {
		let now = Instant::now();
		let entry = self.entry_mut(doc_id);
		entry.sched.last_edit_at = now;
		entry.slot.dirty = true;
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
	) {
		const SYNC_TIMEOUT: Duration = Duration::from_millis(10);

		let now = Instant::now();
		let entry = self.entry_mut(doc_id);
		entry.sched.last_edit_at = now;
		entry.slot.dirty = true;

		let Some(syntax) = entry.slot.current.as_mut() else {
			entry.slot.pending_incremental = None;
			return;
		};

		let version_before = doc_version.wrapping_sub(1);

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
		if let Some(mut entry) = self.entries.remove(&doc_id)
			&& let Some(p) = entry.sched.inflight.take()
		{
			p.task.abort();
		}
	}

	pub fn has_pending(&self, doc_id: DocumentId) -> bool {
		self.entries
			.get(&doc_id)
			.and_then(|d| d.sched.inflight.as_ref())
			.is_some()
	}

	pub fn pending_count(&self) -> usize {
		self.entries
			.values()
			.filter(|d| d.sched.inflight.is_some())
			.count()
	}

	pub fn pending_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.entries
			.iter()
			.filter(|(_, d)| d.sched.inflight.is_some())
			.map(|(id, _)| *id)
	}

	pub fn dirty_docs(&self) -> impl Iterator<Item = DocumentId> + '_ {
		self.entries
			.iter()
			.filter(|(_, e)| e.slot.dirty)
			.map(|(id, _)| *id)
	}

	/// Returns true if any background task has completed its work.
	///
	/// Uses [`JoinHandle::is_finished`] for a non-consuming check. Callers should
	/// usually trigger a redraw when this returns true to ensure [`Self::ensure_syntax`]
	/// installs the result.
	pub fn any_task_finished(&self) -> bool {
		self.entries.values().any(|d| {
			d.sched
				.inflight
				.as_ref()
				.is_some_and(|t| t.task.is_finished())
		})
	}

	/// Drains all completed background tasks into the per-document `completed` slots.
	///
	/// Releases semaphore permits promptly even if the document is not visible.
	pub fn drain_finished_inflight(&mut self) -> bool {
		let mut any_drained = false;
		for entry in self.entries.values_mut() {
			let Some(mut inflight) = entry.sched.inflight.take() else {
				continue;
			};

			if !inflight.task.is_finished() {
				entry.sched.inflight = Some(inflight);
				continue;
			}

			if let Some(res) = xeno_primitives::future::poll_once(&mut inflight.task) {
				entry.sched.completed = Some(CompletedSyntaxTask {
					doc_version: inflight.doc_version,
					lang_id: inflight.lang_id,
					opts: inflight.opts,
					result: res
						.map_err(|e| SyntaxError::Parse(format!("task join error: {}", e)))
						.and_then(|r| r),
				});
				any_drained = true;
			} else {
				entry.sched.inflight = Some(inflight);
			}
		}
		any_drained
	}

	/// Polls or kicks background syntax parsing for a document.
	///
	/// This coordinates polling inflight tasks, enforcing retention and
	/// monotonicity, and spawning new tasks if concurrency permits are available.
	///
	/// # Installation Predicate
	///
	/// Completed parses are installed only if the language and options key still match.
	/// Monotonicity is enforced via `tree_doc_version`.
	pub fn ensure_syntax(&mut self, ctx: EnsureSyntaxContext<'_>) -> SyntaxPollOutcome {
		let now = Instant::now();
		let doc_id = ctx.doc_id;

		// Calculate policy and fetch shared state before borrowing a mutable entry
		let bytes = ctx.content.len_bytes();
		let tier = self.policy.tier_for_bytes(bytes);
		let cfg = self.policy.cfg(tier);
		let current_opts_key = OptKey {
			injections: cfg.injections,
		};

		let permits = Arc::clone(&self.permits);
		let engine = Arc::clone(&self.engine);

		let entry = self.entry_mut(doc_id);
		entry.slot.updated = false;

		// 1. Language/Options change detection
		if entry.slot.language_id != ctx.language_id {
			if let Some(pending) = entry.sched.inflight.take() {
				pending.task.abort();
			}
			entry.slot.current = None;
			entry.slot.tree_doc_version = None;
			entry.sched.completed = None;
			entry.sched.cooldown_until = None;
			entry.slot.dirty = true;
			entry.slot.pending_incremental = None;
			entry.slot.language_id = ctx.language_id;
			mark_updated(&mut entry.slot);
		}

		if matches!(ctx.hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
			entry.sched.last_visible_at = now;
		}

		if entry
			.slot
			.last_opts_key
			.is_some_and(|k| k != current_opts_key)
		{
			if let Some(pending) = entry.sched.inflight.take() {
				pending.task.abort();
			}
			entry.sched.completed = None;
			entry.sched.cooldown_until = None;
			entry.slot.dirty = true;
			entry.slot.pending_incremental = None;
		}
		entry.slot.last_opts_key = Some(current_opts_key);

		// 2. Poll inflight (Internal consistency: don't double-poll if already drained)
		if let Some(mut p) = entry.sched.inflight.take() {
			if let Some(join_res) = xeno_primitives::future::poll_once(&mut p.task) {
				entry.sched.completed = Some(CompletedSyntaxTask {
					doc_version: p.doc_version,
					lang_id: p.lang_id,
					opts: p.opts,
					result: join_res
						.map_err(|e| SyntaxError::Parse(format!("task join error: {}", e)))
						.and_then(|r| r),
				});
			} else {
				entry.sched.inflight = Some(p);
			}
		}

		// 3. Install completed
		if let Some(done) = entry.sched.completed.take() {
			match done.result {
				Ok(syntax_tree) => {
					let Some(current_lang) = ctx.language_id else {
						return SyntaxPollOutcome {
							result: SyntaxPollResult::NoLanguage,
							updated: entry.slot.updated,
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

					let allow_install = should_install_completed_parse(
						done.doc_version,
						entry.slot.tree_doc_version,
						ctx.doc_version,
						entry.slot.dirty,
					);

					if lang_ok && opts_ok && retain_ok && allow_install {
						entry.slot.current = Some(syntax_tree);
						entry.slot.language_id = Some(current_lang);
						entry.slot.tree_doc_version = Some(done.doc_version);
						entry.slot.pending_incremental = None;
						mark_updated(&mut entry.slot);
						if version_match {
							entry.slot.dirty = false;
							entry.sched.cooldown_until = None;
						} else {
							// Stale install: keep dirty true so we eventually catch up
							entry.slot.dirty = true;
						}
					} else if lang_ok && opts_ok && version_match && !retain_ok {
						entry.slot.current = None;
						entry.slot.pending_incremental = None;
						entry.slot.dirty = false;
						mark_updated(&mut entry.slot);
					}
				}
				Err(SyntaxError::Timeout) => {
					entry.sched.cooldown_until = Some(now + cfg.cooldown_on_timeout);
					return SyntaxPollOutcome {
						result: SyntaxPollResult::CoolingDown,
						updated: entry.slot.updated,
					};
				}
				Err(e) => {
					tracing::warn!(?doc_id, ?tier, error=%e, "Background syntax parse failed");
					entry.sched.cooldown_until = Some(now + cfg.cooldown_on_error);
					return SyntaxPollOutcome {
						result: SyntaxPollResult::CoolingDown,
						updated: entry.slot.updated,
					};
				}
			}
		}

		if entry.sched.inflight.is_some() {
			return SyntaxPollOutcome {
				result: SyntaxPollResult::Pending,
				updated: entry.slot.updated,
			};
		}

		// 4. Language check
		let Some(lang_id) = ctx.language_id else {
			if entry.slot.current.is_some() {
				entry.slot.current = None;
				entry.slot.tree_doc_version = None;
				mark_updated(&mut entry.slot);
			}
			entry.slot.language_id = None;
			entry.slot.dirty = false;
			entry.sched.cooldown_until = None;
			return SyntaxPollOutcome {
				result: SyntaxPollResult::NoLanguage,
				updated: entry.slot.updated,
			};
		};

		apply_retention(
			now,
			&entry.sched,
			cfg.retention_hidden,
			ctx.hotness,
			&mut entry.slot,
			doc_id,
		);

		if entry.slot.current.is_some() && !entry.slot.dirty {
			return SyntaxPollOutcome {
				result: SyntaxPollResult::Ready,
				updated: entry.slot.updated,
			};
		}

		// 5. Gating
		if matches!(ctx.hotness, SyntaxHotness::Cold) && !cfg.parse_when_hidden {
			return SyntaxPollOutcome {
				result: SyntaxPollResult::Disabled,
				updated: entry.slot.updated,
			};
		}

		if entry.slot.current.is_some()
			&& now.duration_since(entry.sched.last_edit_at) < cfg.debounce
		{
			return SyntaxPollOutcome {
				result: SyntaxPollResult::Pending,
				updated: entry.slot.updated,
			};
		}

		if let Some(until) = entry.sched.cooldown_until
			&& now < until
		{
			return SyntaxPollOutcome {
				result: SyntaxPollResult::CoolingDown,
				updated: entry.slot.updated,
			};
		}

		// 6. Schedule new task
		let permit = match permits.try_acquire_owned() {
			Ok(p) => p,
			Err(_) => {
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Throttled,
					updated: entry.slot.updated,
				};
			}
		};

		let content = ctx.content.clone();
		let loader = Arc::clone(ctx.loader);

		let opts = SyntaxOptions {
			parse_timeout: cfg.parse_timeout,
			injections: cfg.injections,
		};

		let incremental = match entry.slot.pending_incremental.take() {
			Some(pending)
				if entry.slot.current.is_some()
					&& entry.slot.tree_doc_version == Some(pending.base_tree_doc_version) =>
			{
				Some((entry.slot.current.as_ref().unwrap().clone(), pending))
			}
			Some(_) => None, // Invalid base alignment; fall back to full parse
			None => None,
		};

		let task = tokio::task::spawn_blocking(move || {
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

		entry.sched.inflight = Some(PendingSyntaxTask {
			doc_version: ctx.doc_version,
			lang_id,
			opts: current_opts_key,
			_started_at: now,
			_permit: permit,
			task,
		});

		SyntaxPollOutcome {
			result: SyntaxPollResult::Kicked,
			updated: entry.slot.updated,
		}
	}
}

/// Checks if a completed background parse should be installed into a slot.
///
/// This predicate ensures that a clean incremental tree (produced synchronously
/// during an edit) is not overwritten by a stale full-parse result from a
/// previous document version, and that trees are monotonic (never regress to older versions).
///
/// Installation is permitted if:
/// - The result is not older than the currently resident tree.
/// - The document version matches exactly, OR the slot is currently marked dirty (needs catch-up),
///   OR no syntax tree is currently resident (bootstrap).
fn should_install_completed_parse(
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

/// Bumps the syntax version after a state change.
fn mark_updated(state: &mut SyntaxSlot) {
	state.updated = true;
	state.version = state.version.wrapping_add(1);
}

/// Applies memory retention rules to a syntax slot.
///
/// If a tree is dropped, the dirty flag is cleared to prevent the document from
/// being re-polled while hidden. A bootstrap parse will be triggered once the
/// document becomes visible again.
fn apply_retention(
	now: Instant,
	st: &DocSched,
	policy: RetentionPolicy,
	hotness: SyntaxHotness,
	state: &mut SyntaxSlot,
	_doc_id: DocumentId,
) {
	if matches!(hotness, SyntaxHotness::Visible | SyntaxHotness::Warm) {
		return;
	}

	match policy {
		RetentionPolicy::Keep => {}
		RetentionPolicy::DropWhenHidden => {
			if state.current.is_some() {
				state.current = None;
				state.tree_doc_version = None;
				state.dirty = false;
				state.pending_incremental = None;
				mark_updated(state);
			}
		}
		RetentionPolicy::DropAfter(ttl) => {
			if state.current.is_some() && now.duration_since(st.last_visible_at) > ttl {
				state.current = None;
				state.tree_doc_version = None;
				state.dirty = false;
				state.pending_incremental = None;
				mark_updated(state);
			}
		}
	}
}

#[cfg(test)]
mod tests;
