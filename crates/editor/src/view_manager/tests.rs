use std::path::PathBuf;

use super::ViewManager;
use crate::buffer::{Buffer, ViewId};

#[test]
fn replace_buffer_updates_path_and_doc_indices() {
	let initial = Buffer::new(ViewId(1), "alpha".to_string(), Some(PathBuf::from("a.rs")));
	let old_doc = initial.document_id();
	let mut manager = ViewManager::with_buffer(initial);

	let replacement = Buffer::new(ViewId(1), "beta".to_string(), Some(PathBuf::from("b.rs")));
	let new_doc = replacement.document_id();

	let removed = manager.replace_buffer(ViewId(1), replacement).expect("view must exist");
	assert_eq!(removed.document_id(), old_doc);

	assert_eq!(manager.find_by_path(&PathBuf::from("a.rs")), None);
	assert_eq!(manager.find_by_path(&PathBuf::from("b.rs")), Some(ViewId(1)));
	assert_eq!(manager.any_buffer_for_doc(old_doc), None);
	assert_eq!(manager.any_buffer_for_doc(new_doc), Some(ViewId(1)));
}

#[test]
fn replace_buffer_can_rebind_view_to_existing_document() {
	let first = Buffer::new(ViewId(1), "one".to_string(), Some(PathBuf::from("one.rs")));
	let first_doc = first.document_id();
	let mut manager = ViewManager::with_buffer(first);

	let second = Buffer::new(ViewId(2), "two".to_string(), Some(PathBuf::from("two.rs")));
	let second_doc = second.document_id();
	manager.insert_buffer(ViewId(2), second);

	let replacement = manager.get_buffer(ViewId(2)).expect("source buffer exists").clone_for_split(ViewId(1));
	let removed = manager.replace_buffer(ViewId(1), replacement).expect("view must exist");

	assert_eq!(removed.document_id(), first_doc);
	assert_eq!(manager.any_buffer_for_doc(first_doc), None);
	assert_eq!(manager.views_for_doc(second_doc), &[ViewId(2), ViewId(1)]);
}

#[test]
fn replace_buffer_same_document_does_not_duplicate_index_entry() {
	let first = Buffer::new(ViewId(1), "one".to_string(), Some(PathBuf::from("one.rs")));
	let doc = first.document_id();
	let mut manager = ViewManager::with_buffer(first);

	let replacement = manager.get_buffer(ViewId(1)).expect("buffer exists").clone_for_split(ViewId(1));
	manager.replace_buffer(ViewId(1), replacement).expect("replacement succeeds");

	assert_eq!(manager.views_for_doc(doc), &[ViewId(1)]);
}
