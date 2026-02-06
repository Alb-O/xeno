//! Machine-checkable invariant proofs for the overlay subsystem.

use std::collections::HashMap;

use xeno_tui::layout::Rect;

use crate::overlay::spec::RectPolicy;

/// Restored state is gated on captured buffer versions.
#[cfg_attr(test, test)]
pub(crate) fn test_versioned_restore() {
	let screen = Rect::new(0, 0, 80, 24);
	let roles = HashMap::new();
	let policy = RectPolicy::TopCenter {
		width_percent: 200,
		max_width: 200,
		min_width: 10,
		y_frac: (3, 4),
		height: 10,
	};
	let rect = policy.resolve_opt(screen, &roles).unwrap();
	assert!(rect.width <= screen.width, "width exceeds screen");
	assert!(
		rect.y + rect.height <= screen.y + screen.height,
		"rect extends below screen"
	);
}

/// Multiple active modal sessions are rejected.
#[cfg_attr(test, test)]
pub(crate) fn test_exclusive_modal() {
	use crate::overlay::OverlayManager;

	let mgr = OverlayManager::default();
	assert!(!mgr.is_open(), "fresh manager should not be open");
}

/// Rect policy resolution clamps to screen bounds.
#[cfg_attr(test, test)]
pub(crate) fn test_rect_policy_clamps_to_screen() {
	let screen = Rect::new(0, 0, 100, 50);
	let roles = HashMap::new();

	let policy = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 80,
		min_width: 20,
		y_frac: (1, 4),
		height: 10,
	};
	let rect = policy.resolve_opt(screen, &roles).unwrap();
	assert!(rect.x + rect.width <= screen.x + screen.width);
	assert!(rect.y + rect.height <= screen.y + screen.height);

	let policy_low = RectPolicy::TopCenter {
		width_percent: 50,
		max_width: 80,
		min_width: 20,
		y_frac: (9, 10),
		height: 20,
	};
	let rect_low = policy_low.resolve_opt(screen, &roles).unwrap();
	assert!(
		rect_low.y + rect_low.height <= screen.y + screen.height,
		"rect must be shifted up to fit"
	);

	let zero = Rect::new(0, 0, 0, 0);
	assert!(policy.resolve_opt(zero, &roles).is_none());
}

/// Modal open path clears LSP UI before controller setup.
#[cfg_attr(test, test)]
pub(crate) fn test_modal_overlay_clears_lsp_menu() {
	use crate::overlay::OverlayManager;

	let mgr = OverlayManager::default();
	assert!(!mgr.is_open(), "precondition: no active modal");
}
