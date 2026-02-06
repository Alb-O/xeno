//! Machine-checkable invariant catalog and proof entrypoints for syntax scheduling.
#![allow(dead_code)]

pub(crate) mod catalog;

#[allow(unused_imports)]
pub(crate) use catalog::{
	BOOTSTRAP_PARSE_SKIPS_DEBOUNCE, HIGHLIGHT_RENDERING_SKIPS_STALE_TREE_VERSION,
	MONOTONIC_STALE_INSTALL_GUARD, MUTATIONS_MUST_NOTE_EDIT, NO_UNBOUNDED_UI_THREAD_PARSING,
	PENDING_INCREMENTAL_CLEARED_ON_INVALIDATIONS, PERMIT_LIFETIME_TIED_TO_TASK_EXECUTION,
	RECENTLY_VISIBLE_PROMOTED_TO_WARM, SINGLE_FLIGHT_PER_DOCUMENT,
	SYNTAX_VERSION_BUMPS_ON_TREE_CHANGE, TICK_MUST_DRAIN_COMPLETED_INFLIGHT,
	TREE_DOC_VERSION_TRACKED_AND_CLEARED,
};

#[cfg(doc)]
pub(crate) async fn test_single_flight_per_doc() {}

#[cfg(doc)]
pub(crate) async fn test_inflight_drained_even_if_doc_marked_clean() {}

#[cfg(doc)]
pub(crate) async fn test_stale_parse_does_not_overwrite_clean_incremental() {}

#[cfg(doc)]
pub(crate) async fn test_stale_install_continuity() {}

#[cfg(doc)]
pub(crate) async fn test_note_edit_updates_timestamp() {}

#[cfg(doc)]
pub(crate) async fn test_bootstrap_parse_skips_debounce() {}

#[cfg(doc)]
pub(crate) async fn test_idle_tick_polls_inflight_parse() {}

#[cfg(doc)]
pub(crate) async fn test_syntax_version_bumps_on_install() {}

#[cfg(doc)]
pub(crate) async fn test_language_switch_discards_old_parse() {}

#[cfg(doc)]
pub(crate) async fn test_invalidate_does_not_release_permit_until_task_finishes() {}

#[cfg(doc)]
pub(crate) async fn test_highlight_skips_stale_tree_version() {}

#[cfg(doc)]
pub(crate) fn test_warm_hotness_prevents_immediate_drop() {}

#[cfg(test)]
mod proofs;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use proofs::{
	EngineGuard, MockEngine, test_bootstrap_parse_skips_debounce,
	test_highlight_skips_stale_tree_version, test_idle_tick_polls_inflight_parse,
	test_inflight_drained_even_if_doc_marked_clean,
	test_invalidate_does_not_release_permit_until_task_finishes,
	test_language_switch_discards_old_parse, test_note_edit_updates_timestamp,
	test_single_flight_per_doc, test_stale_install_continuity,
	test_stale_parse_does_not_overwrite_clean_incremental, test_syntax_version_bumps_on_install,
	test_warm_hotness_prevents_immediate_drop,
};
