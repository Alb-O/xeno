//! Machine-checkable invariant catalog and proof entrypoints for overlay behavior.
#![allow(dead_code)]

pub(crate) mod catalog;

#[allow(unused_imports)]
pub(crate) use catalog::{
	CLAMP_RESOLVED_AREAS_TO_SCREEN_BOUNDS, CLEAR_LSP_UI_WHEN_MODAL_OPENS,
	ONLY_ONE_ACTIVE_MODAL_SESSION, RESTORE_ONLY_WHEN_CAPTURE_VERSION_MATCHES,
};

#[cfg(doc)]
pub(crate) fn test_versioned_restore() {}

#[cfg(doc)]
pub(crate) fn test_exclusive_modal() {}

#[cfg(doc)]
pub(crate) fn test_rect_policy_clamps_to_screen() {}

#[cfg(doc)]
pub(crate) fn test_modal_overlay_clears_lsp_menu() {}

#[cfg(test)]
mod proofs;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use proofs::{
	test_exclusive_modal, test_modal_overlay_clears_lsp_menu, test_rect_policy_clamps_to_screen,
	test_versioned_restore,
};
