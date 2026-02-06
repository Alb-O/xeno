//! Invariant catalog for [`crate::syntax_manager::SyntaxManager`].
#![allow(dead_code)]

/// Must not perform unbounded parsing on the UI thread.
///
/// - Enforced in: [`crate::syntax_manager::SyntaxManager::ensure_syntax`] and [`crate::syntax_manager::SyntaxManager::note_edit_incremental`]
/// - Tested by: [`crate::syntax_manager::invariants::test_inflight_drained_even_if_doc_marked_clean`]
/// - Failure symptom: UI freezes or jitters during edits.
pub(crate) const NO_UNBOUNDED_UI_THREAD_PARSING: () = ();

/// Must enforce single-flight per document.
///
/// - Enforced in: [`crate::syntax_manager::SyntaxManager::ensure_syntax`]
/// - Tested by: [`crate::syntax_manager::invariants::test_single_flight_per_doc`]
/// - Failure symptom: Multiple redundant parse tasks for the same document identity.
pub(crate) const SINGLE_FLIGHT_PER_DOCUMENT: () = ();

/// Must install the last completed parse when needed for continuity, but must not regress to
/// a tree older than the currently installed `tree_doc_version`.
///
/// - Enforced in: [`crate::syntax_manager::should_install_completed_parse`]
/// - Tested by: [`crate::syntax_manager::invariants::test_stale_parse_does_not_overwrite_clean_incremental`], [`crate::syntax_manager::invariants::test_stale_install_continuity`]
/// - Failure symptom: Stale trees overwrite newer incrementals, or highlighting stays missing until an exact-version parse completes.
pub(crate) const MONOTONIC_STALE_INSTALL_GUARD: () = ();

/// Must call [`crate::syntax_manager::SyntaxManager::note_edit_incremental`] (or
/// [`crate::syntax_manager::SyntaxManager::note_edit`]) on every document mutation.
///
/// - Enforced in: `EditorUndoHost::apply_transaction_inner`, `EditorUndoHost::apply_history_op`, `Editor::apply_buffer_edit_plan`
/// - Tested by: [`crate::syntax_manager::invariants::test_note_edit_updates_timestamp`]
/// - Failure symptom: Debounce in [`crate::syntax_manager::SyntaxManager::ensure_syntax`] is bypassed and background parses run without edit silence.
pub(crate) const MUTATIONS_MUST_NOTE_EDIT: () = ();

/// Must skip debounce for bootstrap parses when no syntax tree is installed.
///
/// - Enforced in: [`crate::syntax_manager::SyntaxManager::ensure_syntax`]
/// - Tested by: [`crate::syntax_manager::invariants::test_bootstrap_parse_skips_debounce`]
/// - Failure symptom: Newly opened documents remain unhighlighted until debounce elapses.
pub(crate) const BOOTSTRAP_PARSE_SKIPS_DEBOUNCE: () = ();

/// Must detect completed inflight syntax tasks from `tick()`, not only from `render()`.
///
/// - Enforced in: [`crate::syntax_manager::SyntaxManager::drain_finished_inflight`] via `Editor::tick`
/// - Tested by: [`crate::syntax_manager::invariants::test_idle_tick_polls_inflight_parse`]
/// - Failure symptom: Completed parses are not installed while idle until user input triggers rendering.
pub(crate) const TICK_MUST_DRAIN_COMPLETED_INFLIGHT: () = ();

/// Must bump `syntax_version` whenever the installed tree changes or is dropped.
///
/// - Enforced in: [`crate::syntax_manager::mark_updated`]
/// - Tested by: [`crate::syntax_manager::invariants::test_syntax_version_bumps_on_install`]
/// - Failure symptom: Highlight cache serves stale spans after reparse or retention drop.
pub(crate) const SYNTAX_VERSION_BUMPS_ON_TREE_CHANGE: () = ();

/// Must clear `pending_incremental` on language change, syntax reset, and retention drop.
///
/// - Enforced in: [`crate::syntax_manager::SyntaxManager::ensure_syntax`], [`crate::syntax_manager::SyntaxManager::reset_syntax`], [`crate::syntax_manager::apply_retention`]
/// - Tested by: [`crate::syntax_manager::invariants::test_language_switch_discards_old_parse`]
/// - Failure symptom: Stale changesets are applied against mismatched ropes, causing bad edits or panics.
pub(crate) const PENDING_INCREMENTAL_CLEARED_ON_INVALIDATIONS: () = ();

/// Must track `tree_doc_version` with the installed tree, and must clear it when the tree is
/// dropped.
///
/// - Enforced in: [`crate::syntax_manager::SyntaxManager::note_edit_incremental`], [`crate::syntax_manager::SyntaxManager::ensure_syntax`], [`crate::syntax_manager::SyntaxManager::reset_syntax`], [`crate::syntax_manager::apply_retention`]
/// - Tested by: [`crate::syntax_manager::invariants::test_stale_parse_does_not_overwrite_clean_incremental`]
/// - Failure symptom: Rendering uses a tree from the wrong document version, causing garbled highlights or bounds bugs.
pub(crate) const TREE_DOC_VERSION_TRACKED_AND_CLEARED: () = ();

/// Highlight rendering must skip spans when `tree_doc_version` differs from the rendered document
/// version.
///
/// - Enforced in: `HighlightTiles::build_tile_spans` (in `crate::render::cache::highlight`)
/// - Tested by: [`crate::syntax_manager::invariants::test_highlight_skips_stale_tree_version`]
/// - Failure symptom: Out-of-bounds tree-sitter access can panic during rapid edits.
pub(crate) const HIGHLIGHT_RENDERING_SKIPS_STALE_TREE_VERSION: () = ();

/// Recently visible documents must be promoted to `Warm` hotness using
/// [`crate::syntax_manager::lru::RecentDocLru`] to avoid immediate retention drops.
///
/// - Enforced in: `Editor::ensure_syntax_for_buffers`, `Editor::on_document_close`
/// - Tested by: [`crate::syntax_manager::invariants::test_warm_hotness_prevents_immediate_drop`]
/// - Failure symptom: Switching away for one frame drops syntax and causes a flash of unhighlighted text.
pub(crate) const RECENTLY_VISIBLE_PROMOTED_TO_WARM: () = ();

/// Must tie background task permit lifetime to real thread execution.
///
/// - Enforced in: [`crate::syntax_manager::TaskCollector::spawn`]
/// - Tested by: [`crate::syntax_manager::invariants::test_invalidate_does_not_release_permit_until_task_finishes`]
/// - Failure symptom: Concurrency cap is violated under churn because permits are released before CPU work ends.
pub(crate) const PERMIT_LIFETIME_TIED_TO_TASK_EXECUTION: () = ();
