use std::collections::HashMap;
pub mod lru;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use xeno_primitives::{ChangeSet, Rope};
use xeno_runtime_language::syntax::{
	InjectionPolicy, SealedSource, Syntax, SyntaxError, SyntaxOptions,
};
use xeno_runtime_language::{LanguageId, LanguageLoader};

use crate::core::document::DocumentId;

const DEFAULT_MAX_CONCURRENCY: usize = 2;
const VIEWPORT_LOOKBEHIND: u32 = 8192;
const VIEWPORT_LOOKAHEAD: u32 = 8192;
const VIEWPORT_WINDOW_MAX: u32 = 128 * 1024;

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
pub struct OptKey {
	pub injections: InjectionPolicy,
}

/// Source of a document edit, used to determine scheduling priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditSource {
	/// Interactive typing or local edit (debounced).
	Typing,
	/// Undo/redo or bulk operation (immediate).
	History,
}

/// Generation counter for a document's syntax state.
///
/// Incremented on language changes or syntax resets to invalidate stale background results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct DocEpoch(u64);

impl DocEpoch {
	pub fn next(self) -> Self {
		Self(self.0.wrapping_add(1))
	}
}

/// Unique identifier for a background syntax task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

pub(crate) struct DocSched {
	epoch: DocEpoch,
	last_edit_at: Instant,
	last_visible_at: Instant,
	cooldown_until: Option<Instant>,
	active_task: Option<TaskId>,
	active_task_detached: bool,
	completed: Option<CompletedSyntaxTask>,
	/// If true, bypasses the debounce gate for the next background parse.
	force_no_debounce: bool,
}

impl DocSched {
	fn new(now: Instant) -> Self {
		Self {
			epoch: DocEpoch(0),
			last_edit_at: now,
			last_visible_at: now,
			cooldown_until: None,
			active_task: None,
			active_task_detached: false,
			completed: None,
			force_no_debounce: false,
		}
	}

	/// Invalidates the current scheduling window, bumping the epoch to discard stale tasks.
	///
	/// NOTE: Invalidation does not imply cancellation of the background thread; permits
	/// are released only on task completion to maintain strict concurrency bounds.
	fn invalidate(&mut self) {
		self.epoch = self.epoch.next();
		self.active_task = None;
		self.active_task_detached = false;
		self.completed = None;
		self.cooldown_until = None;
		self.force_no_debounce = false;
	}
}

enum TaskKind {
	FullParse {
		content: Rope,
	},
	ViewportParse {
		content: Rope,
		window: std::ops::Range<u32>,
	},
	Incremental {
		base: Syntax,
		old_rope: Rope,
		new_rope: Rope,
		composed: ChangeSet,
	},
}

struct TaskSpec {
	doc_id: DocumentId,
	epoch: DocEpoch,
	doc_version: u64,
	lang_id: LanguageId,
	opts_key: OptKey,
	opts: SyntaxOptions,
	kind: TaskKind,
	loader: Arc<LanguageLoader>,
}

struct TaskDone {
	id: TaskId,
	doc_id: DocumentId,
	epoch: DocEpoch,
	doc_version: u64,
	lang_id: LanguageId,
	opts_key: OptKey,
	result: Result<Syntax, SyntaxError>,
	is_viewport: bool,
}

/// Invariant enforcement: Collector for background syntax tasks.
pub(crate) struct TaskCollector {
	next_id: u64,
	tasks: FxHashMap<u64, JoinHandle<TaskDone>>,
}

impl TaskCollector {
	fn new() -> Self {
		Self {
			next_id: 0,
			tasks: FxHashMap::default(),
		}
	}

	fn spawn(
		&mut self,
		permits: Arc<Semaphore>,
		engine: Arc<dyn SyntaxEngine>,
		spec: TaskSpec,
	) -> Option<TaskId> {
		let permit = permits.try_acquire_owned().ok()?;
		let id_val = self.next_id;
		self.next_id = self.next_id.wrapping_add(1);
		let task_id = TaskId(id_val);

		let is_viewport = matches!(spec.kind, TaskKind::ViewportParse { .. });

		let handle = tokio::task::spawn_blocking(move || {
			let _permit = permit; // Tie permit lifetime to closure

			let result = match spec.kind {
				TaskKind::FullParse { content } => {
					engine.parse(content.slice(..), spec.lang_id, &spec.loader, spec.opts)
				}
				TaskKind::ViewportParse { content, window } => {
					if let Some(data) = spec.loader.get(spec.lang_id) {
						let repair = data.viewport_repair();
						let forward_haystack = if window.end < content.len_bytes() as u32 {
							Some(content.byte_slice(window.end as usize..))
						} else {
							None
						};
						let suffix = repair.scan(
							content.byte_slice(window.start as usize..window.end as usize),
							forward_haystack,
						);
						let sealed = Arc::new(SealedSource::from_window(
							content.byte_slice(window.start as usize..window.end as usize),
							&suffix,
						));
						Syntax::new_viewport(
							sealed,
							spec.lang_id,
							&spec.loader,
							spec.opts,
							window.start,
						)
					} else {
						Err(SyntaxError::NoLanguage)
					}
				}
				TaskKind::Incremental {
					base,
					old_rope,
					new_rope,
					composed,
				} => engine.update_incremental(
					base,
					old_rope.slice(..),
					new_rope.slice(..),
					&composed,
					spec.lang_id,
					&spec.loader,
					spec.opts,
				),
			};

			TaskDone {
				id: task_id,
				doc_id: spec.doc_id,
				epoch: spec.epoch,
				doc_version: spec.doc_version,
				lang_id: spec.lang_id,
				opts_key: spec.opts_key,
				result,
				is_viewport,
			}
		});

		self.tasks.insert(id_val, handle);
		Some(task_id)
	}

	fn drain_finished(&mut self) -> Vec<TaskDone> {
		let mut done = Vec::new();

		self.tasks.retain(|_, handle| {
			match xeno_primitives::future::poll_once(handle) {
				None => true, // Still running, keep it
				Some(Ok(task_done)) => {
					done.push(task_done);
					false // Done, remove it
				}
				Some(Err(e)) => {
					tracing::error!("Syntax task join error: {}", e);
					false // Done (crashed), remove it
				}
			}
		});

		done
	}

	fn any_finished(&self) -> bool {
		self.tasks.values().any(|h| h.is_finished())
	}
}

struct CompletedSyntaxTask {
	doc_version: u64,
	lang_id: LanguageId,
	opts: OptKey,
	result: Result<Syntax, SyntaxError>,
	is_viewport: bool,
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
		_lang: LanguageId,
		loader: &LanguageLoader,
		opts: SyntaxOptions,
	) -> Result<Syntax, SyntaxError> {
		syntax
			.update_from_changeset(old_source, new_source, changeset, loader, opts)
			.map(|()| syntax)
			.or_else(|e| {
				tracing::warn!(error = %e, "Incremental parse failed, falling back to full reparse");
				Syntax::new(new_source, _lang, loader, opts)
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
	/// Collector for background tasks.
	collector: TaskCollector,
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
	pub viewport: Option<std::ops::Range<u32>>,
}

/// Invariant enforcement: Accumulated edits awaiting an incremental reparse.
pub(crate) struct PendingIncrementalEdits {
	/// Document version that `old_rope` corresponds to.
	///
	/// Used to verify that the resident tree still matches the pending base
	/// before attempting an incremental update.
	pub(crate) base_tree_doc_version: u64,
	/// Source text at the start of the pending edit window.
	pub(crate) old_rope: Rope,
	/// Composed delta from `old_rope` to the current document state.
	pub(crate) composed: ChangeSet,
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
	/// Coverage of the currently installed tree (doc-global bytes).
	pub coverage: Option<std::ops::Range<u32>>,
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
			collector: TaskCollector::new(),
		}
	}

	#[cfg(any(test, doc))]
	pub fn new_with_engine(max_concurrency: usize, engine: Arc<dyn SyntaxEngine>) -> Self {
		Self {
			policy: TieredSyntaxPolicy::default(),
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
					Err(SyntaxError::Timeout) => {
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
				tracing::debug!(
					?doc_id,
					?_task_id,
					"Syntax task already active; returning Pending"
				);
				return SyntaxPollOutcome {
					result: SyntaxPollResult::Pending,
					updated: was_updated,
				};
			}
		}

		// 6. Schedule new task
		let lang_id = ctx.language_id.unwrap();

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

impl SyntaxSlot {
	pub fn take_updated(&mut self) -> bool {
		let res = self.updated;
		self.updated = false;
		res
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
