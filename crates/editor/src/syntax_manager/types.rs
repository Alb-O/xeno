use std::collections::VecDeque;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use xeno_language::syntax::{InjectionPolicy, Syntax};
use xeno_language::{LanguageId, LanguageLoader};
use xeno_primitives::{ChangeSet, Rope};

use crate::core::document::DocumentId;

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
pub struct DocEpoch(pub(super) u64);

impl DocEpoch {
	pub(super) fn next(self) -> Self {
		Self(self.0.wrapping_add(1))
	}
}

/// Unique identifier for a background syntax task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub(super) u64);

/// Context provided to [`crate::syntax_manager::SyntaxManager::ensure_syntax`] for scheduling.
pub struct EnsureSyntaxContext<'a> {
	pub doc_id: DocumentId,
	pub doc_version: u64,
	pub language_id: Option<LanguageId>,
	pub content: &'a Rope,
	pub hotness: super::SyntaxHotness,
	pub loader: &'a Arc<LanguageLoader>,
	pub viewport: Option<std::ops::Range<u32>>,
}

/// Mapping context for projecting stale tree-based highlights onto current text.
///
/// When the resident syntax tree lags behind the current document version,
/// highlight spans can be remapped through `composed_changes` to preserve
/// visual attachment to the edited text during debounce/catch-up windows.
#[derive(Clone, Copy)]
pub struct HighlightProjectionCtx<'a> {
	/// Document version the resident tree corresponds to.
	pub tree_doc_version: u64,
	/// Current target document version for rendering.
	pub target_doc_version: u64,
	/// Rope snapshot at the start of the pending edit window.
	pub base_rope: &'a Rope,
	/// Composed delta from `base_rope` to the current rope.
	pub composed_changes: &'a ChangeSet,
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

/// Best-available syntax tree selected for rendering a viewport.
///
/// Returned by [`crate::syntax_manager::SyntaxManager::syntax_for_viewport`]
/// and consumed by the highlight cache. The `tree_id` serves as the cache
/// key for highlight tiles, ensuring correct invalidation across tree swaps
/// and in-place full-tree incremental updates.
pub struct SyntaxSelection<'a> {
	pub syntax: &'a Syntax,
	/// Unique per-tree-state identity (monotonic within a document slot).
	pub tree_id: u64,
	/// Document version the tree was parsed from.
	pub tree_doc_version: u64,
	/// Byte coverage if this is a partial (viewport) tree; `None` for full trees.
	pub coverage: Option<std::ops::Range<u32>>,
}

/// Stable key for viewport window cache entries. Aligned to a stride for reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewportKey(pub u32);

/// A cached viewport syntax tree at a specific stage.
pub struct ViewportTree {
	pub(super) syntax: Syntax,
	pub(super) doc_version: u64,
	pub(super) tree_id: u64,
	pub(super) coverage: std::ops::Range<u32>,
}

/// Cache entry for a single viewport window, holding Stage-A and Stage-B trees.
pub struct ViewportEntry {
	pub(super) key: ViewportKey,
	/// Stage-A tree (fast, injections matching tier config).
	pub(super) stage_a: Option<ViewportTree>,
	/// Doc version for which Stage-A failed (timeout/error) in this key.
	///
	/// Used to suppress same-version history urgent retries so background catch-up
	/// can proceed instead of looping Stage-A timeouts.
	pub(super) stage_a_failed_for: Option<u64>,
	/// Stage-B tree (injection-eager enrichment).
	pub(super) stage_b: Option<ViewportTree>,
	/// Doc version for which Stage-B was already attempted (per-window latch).
	pub(super) attempted_b_for: Option<u64>,
	/// Per-key cooldown after Stage-B timeout/error.
	pub(super) stage_b_cooldown_until: Option<std::time::Instant>,
}

/// Small LRU cache of viewport parse results per document.
///
/// Prevents scroll-thrash by retaining recently-visited viewport windows
/// so that scrolling back reuses already-parsed regions instead of reparsing.
pub struct ViewportCache {
	cap: usize,
	order: VecDeque<ViewportKey>,
	pub(super) map: FxHashMap<ViewportKey, ViewportEntry>,
}

impl Default for ViewportCache {
	fn default() -> Self {
		Self::new(4)
	}
}

impl ViewportCache {
	pub fn new(cap: usize) -> Self {
		Self {
			cap,
			order: VecDeque::with_capacity(cap),
			map: FxHashMap::default(),
		}
	}

	/// Iterates keys in MRU order (most recently used first).
	pub fn iter_keys_mru(&self) -> impl Iterator<Item = ViewportKey> + '_ {
		self.order.iter().copied()
	}

	/// Returns all entries whose coverage overlaps the given byte range.
	pub fn get_overlapping(&self, viewport: &std::ops::Range<u32>) -> impl Iterator<Item = &ViewportEntry> {
		self.map.values().filter(move |e| {
			let covers = |t: &ViewportTree| t.coverage.start < viewport.end && t.coverage.end > viewport.start;
			e.stage_a.as_ref().is_some_and(covers) || e.stage_b.as_ref().is_some_and(covers)
		})
	}

	/// Gets or creates an entry for the given key, touching it as MRU.
	pub fn get_mut_or_insert(&mut self, key: ViewportKey) -> &mut ViewportEntry {
		if self.map.contains_key(&key) {
			self.touch(key);
		} else {
			if self.order.len() >= self.cap {
				if let Some(evicted) = self.order.pop_back() {
					self.map.remove(&evicted);
				}
			}
			self.order.push_front(key);
			self.map.insert(
				key,
				ViewportEntry {
					key,
					stage_a: None,
					stage_a_failed_for: None,
					stage_b: None,
					attempted_b_for: None,
					stage_b_cooldown_until: None,
				},
			);
		}
		self.map.get_mut(&key).unwrap()
	}

	/// Moves a key to the front of the MRU order.
	pub fn touch(&mut self, key: ViewportKey) {
		if let Some(pos) = self.order.iter().position(|k| *k == key) {
			self.order.remove(pos);
			self.order.push_front(key);
		}
	}

	/// Returns true if any cached tree fully covers the given byte range.
	pub fn covers_range(&self, vp: &std::ops::Range<u32>) -> bool {
		self.covering_key(vp).is_some()
	}

	/// Returns the best covering key in MRU order, preferring entries with stage_b.
	///
	/// Scans keys front-to-back (most recently used first). Among covering entries,
	/// prefers one with a stage_b tree (eager injections). If multiple cover with
	/// stage_b, the MRU one wins.
	pub fn covering_key(&self, vp: &std::ops::Range<u32>) -> Option<ViewportKey> {
		let covers = |t: &ViewportTree| t.coverage.start <= vp.start && t.coverage.end >= vp.end;
		let mut best: Option<ViewportKey> = None;
		let mut best_has_b = false;

		for &key in &self.order {
			let Some(entry) = self.map.get(&key) else { continue };
			let a_covers = entry.stage_a.as_ref().is_some_and(&covers);
			let b_covers = entry.stage_b.as_ref().is_some_and(&covers);
			if !a_covers && !b_covers {
				continue;
			}
			if b_covers && !best_has_b {
				best = Some(key);
				best_has_b = true;
			} else if best.is_none() {
				best = Some(key);
			}
		}
		best
	}

	/// Clears all cached viewport trees.
	pub fn clear(&mut self) {
		self.order.clear();
		self.map.clear();
	}

	/// Returns true if any tree is cached.
	pub fn has_any(&self) -> bool {
		self.map.values().any(|e| e.stage_a.is_some() || e.stage_b.is_some())
	}

	/// Returns the best doc version across all cached viewport trees.
	pub fn best_doc_version(&self) -> Option<u64> {
		self.map
			.values()
			.filter_map(|e| {
				let a = e.stage_a.as_ref().map(|t| t.doc_version);
				let b = e.stage_b.as_ref().map(|t| t.doc_version);
				a.max(b)
			})
			.max()
	}
}

/// Per-document syntax state managed by [`crate::syntax_manager::SyntaxManager`].
///
/// Maintains two independent tree slots: a full-document tree and a
/// viewport-bounded partial tree. Rendering selects the best available
/// tree via [`crate::syntax_manager::SyntaxManager::syntax_for_viewport`],
/// preventing viewport installs from clobbering a valid full tree.
#[derive(Default)]
pub struct SyntaxSlot {
	/// Full-document syntax tree (may have injections enabled).
	pub(super) full: Option<Syntax>,
	/// Document version the full tree was parsed from.
	pub(super) full_doc_version: Option<u64>,
	/// Unique identity for the current full tree state.
	pub(super) full_tree_id: u64,

	/// LRU cache of viewport-bounded partial syntax trees keyed by aligned window.
	pub(super) viewport_cache: ViewportCache,

	/// Whether the document has been edited since the last successful parse.
	pub(super) dirty: bool,
	/// Whether any tree was updated in the last poll.
	pub(super) updated: bool,
	/// Monotonic change counter, bumped whenever any tree changes or is dropped.
	/// Used as the highlight cache invalidation key.
	pub(super) change_id: u64,
	/// Generator for unique tree IDs within this document.
	pub(super) next_tree_id: u64,
	/// Language identity used for the last parse.
	pub(super) language_id: Option<LanguageId>,
	/// Accumulated incremental edits awaiting background processing.
	pub(super) pending_incremental: Option<PendingIncrementalEdits>,
	/// Configuration options used for the last parse.
	pub(super) last_opts_key: Option<OptKey>,
	/// Whether a synchronous bootstrap parse has already been attempted.
	pub(crate) sync_bootstrap_attempted: bool,
}

impl SyntaxSlot {
	pub fn take_updated(&mut self) -> bool {
		let res = self.updated;
		self.updated = false;
		res
	}

	/// Allocates a new unique tree ID for this slot.
	pub(super) fn alloc_tree_id(&mut self) -> u64 {
		let id = self.next_tree_id;
		self.next_tree_id = self.next_tree_id.wrapping_add(1);
		id
	}

	/// Returns true if any syntax tree is installed (full or viewport cache).
	pub(super) fn has_any_tree(&self) -> bool {
		self.full.is_some() || self.viewport_cache.has_any()
	}

	/// Returns the document version of the best available tree.
	pub(super) fn best_doc_version(&self) -> Option<u64> {
		let vp_best = self.viewport_cache.best_doc_version();
		match (self.full_doc_version, vp_best) {
			(Some(f), Some(v)) => Some(f.max(v)),
			(f, v) => f.or(v),
		}
	}

	/// Drops the full tree and resets associated state.
	pub(super) fn drop_full(&mut self) {
		self.full = None;
		self.full_doc_version = None;
	}

	/// Drops all viewport cached trees.
	pub(super) fn drop_viewport(&mut self) {
		self.viewport_cache.clear();
	}

	/// Wholesale drops all resident syntax trees and resets all tree-coupled latches.
	pub(crate) fn drop_tree(&mut self) {
		self.drop_full();
		self.drop_viewport();
		self.pending_incremental = None;
		self.sync_bootstrap_attempted = false;
	}
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
}

pub struct SyntaxPollOutcome {
	pub result: SyntaxPollResult,
	pub updated: bool,
}
