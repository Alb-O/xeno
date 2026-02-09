use std::sync::Arc;

use xeno_primitives::{ChangeSet, Rope};
use xeno_runtime_language::syntax::{InjectionPolicy, Syntax};
use xeno_runtime_language::{LanguageId, LanguageLoader};

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

/// Per-document syntax state managed by [`crate::syntax_manager::SyntaxManager`].
///
/// Tracks the installed syntax tree, its document version, and pending
/// incremental edits. The `tree_doc_version` field enables monotonic
/// version gating to prevent stale parse results from overwriting newer trees.
#[derive(Default)]
pub struct SyntaxSlot {
	/// Currently installed syntax tree, if any.
	pub(super) current: Option<Syntax>,
	/// Whether the document has been edited since the last successful parse.
	pub(super) dirty: bool,
	/// Whether the `current` tree was updated in the last poll.
	pub(super) updated: bool,
	/// Local version counter, bumped whenever `current` changes or is dropped.
	pub(super) version: u64,
	/// Document version that the `current` syntax tree corresponds to.
	///
	/// Set by `note_edit_incremental` (on sync success) and `ensure_syntax` (on
	/// background install). `None` means no tree is installed.
	pub(super) tree_doc_version: Option<u64>,
	/// Language identity used for the last parse.
	pub(super) language_id: Option<LanguageId>,
	/// Accumulated incremental edits awaiting background processing.
	pub(super) pending_incremental: Option<PendingIncrementalEdits>,
	/// Configuration options used for the last parse.
	pub(super) last_opts_key: Option<OptKey>,
	/// Coverage of the currently installed tree (doc-global bytes).
	pub coverage: Option<std::ops::Range<u32>>,
	/// Whether a synchronous bootstrap parse has already been attempted.
	///
	/// Reset when the tree is dropped or changed to allow another attempt.
	pub(crate) sync_bootstrap_attempted: bool,
	/// Whether Stage B viewport parsing (with injections) has already been attempted
	/// for the current tree.
	pub(crate) viewport_stage_b_attempted: bool,
}

impl SyntaxSlot {
	pub fn take_updated(&mut self) -> bool {
		let res = self.updated;
		self.updated = false;
		res
	}

	/// Wholesale drops the resident syntax tree and resets all tree-coupled latches.
	pub(crate) fn drop_tree(&mut self) {
		self.current = None;
		self.tree_doc_version = None;
		self.coverage = None;
		self.pending_incremental = None;
		self.sync_bootstrap_attempted = false;
		self.viewport_stage_b_attempted = false;
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
	/// Throttled by global concurrency cap.
	Throttled,
}

pub struct SyntaxPollOutcome {
	pub result: SyntaxPollResult,
	pub updated: bool,
}
