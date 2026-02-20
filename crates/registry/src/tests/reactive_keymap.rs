use std::sync::Arc;

use crate::db::keymap_registry::get_keymap_snapshot;

#[test]
fn test_keymap_snapshot_is_stable_for_immutable_actions_catalog() {
	let first = get_keymap_snapshot();
	let second = get_keymap_snapshot();
	assert!(Arc::ptr_eq(&first, &second), "immutable action catalog should reuse the same keymap snapshot");
}
