//! Invariant catalog for [`crate::overlay::OverlaySystem`].
#![allow(dead_code)]

/// Must restore captured state only when buffer versions still match.
///
/// - Enforced in: [`crate::overlay::session::OverlaySession::restore_all`]
/// - Tested by: [`crate::overlay::invariants::test_versioned_restore`]
/// - Failure symptom: User edits are clobbered by stale preview restoration.
pub(crate) const RESTORE_ONLY_WHEN_CAPTURE_VERSION_MATCHES: () = ();

/// Must not allow multiple active modal sessions.
///
/// - Enforced in: [`crate::overlay::OverlayManager::open`]
/// - Tested by: [`crate::overlay::invariants::test_exclusive_modal`]
/// - Failure symptom: Multiple modal prompts fight for focus and key handling.
pub(crate) const ONLY_ONE_ACTIVE_MODAL_SESSION: () = ();

/// Must clamp resolved window areas to screen bounds.
///
/// - Enforced in: [`crate::overlay::spec::RectPolicy::resolve_opt`]
/// - Tested by: [`crate::overlay::invariants::test_rect_policy_clamps_to_screen`]
/// - Failure symptom: Overlay windows render off-screen or collapse to invalid sizes.
pub(crate) const CLAMP_RESOLVED_AREAS_TO_SCREEN_BOUNDS: () = ();

/// Must clear LSP UI when opening a modal overlay.
///
/// - Enforced in: [`crate::overlay::OverlayManager::open`]
/// - Tested by: [`crate::overlay::invariants::test_modal_overlay_clears_lsp_menu`]
/// - Failure symptom: Completion or other LSP UI leaks above modal prompts.
pub(crate) const CLEAR_LSP_UI_WHEN_MODAL_OPENS: () = ();
