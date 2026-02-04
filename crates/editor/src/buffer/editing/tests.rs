use xeno_runtime_language::LanguageLoader;

use crate::buffer::{ApplyPolicy, Buffer, ViewId};

#[test]
fn readonly_flag_roundtrip() {
	let buffer = Buffer::scratch(ViewId::SCRATCH);
	assert!(!buffer.is_readonly());
	buffer.set_readonly(true);
	assert!(buffer.is_readonly());
}

#[test]
fn readonly_blocks_apply_transaction() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	let (tx, _selection) = buffer.prepare_insert("hi");
	buffer.set_readonly(true);
	let result = buffer.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
	assert!(!result.applied);
	assert_eq!(buffer.with_doc(|doc| doc.content().to_string()), "");
}

#[test]
fn readonly_override_blocks_transaction() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	assert!(!buffer.with_doc(|doc| doc.is_readonly()));
	buffer.set_readonly_override(Some(true));
	assert!(buffer.is_readonly());

	let (tx, _selection) = buffer.prepare_insert("hi");
	let result = buffer.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
	assert!(!result.applied);
	assert_eq!(buffer.with_doc(|doc| doc.content().to_string()), "");
}

#[test]
fn readonly_override_allows_write_on_readonly_doc() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	// Document is readonly, but buffer override makes it writable
	buffer.set_readonly(true);
	assert!(buffer.is_readonly());

	buffer.set_readonly_override(Some(false));
	assert!(!buffer.is_readonly());

	let (tx, _selection) = buffer.prepare_insert("hi");
	let result = buffer.apply(&tx, ApplyPolicy::BARE, &LanguageLoader::new());
	assert!(result.applied);
	assert_eq!(buffer.with_doc(|doc| doc.content().to_string()), "hi");
}

#[test]
fn readonly_override_none_defers_to_document() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	buffer.set_readonly_override(None);
	assert!(!buffer.is_readonly()); // Document is writable

	buffer.set_readonly(true);
	assert!(buffer.is_readonly()); // Now document is readonly, override defers
}

#[test]
fn split_does_not_inherit_readonly_override() {
	let mut buffer = Buffer::scratch(ViewId::SCRATCH);
	buffer.set_readonly_override(Some(true));
	assert!(buffer.is_readonly());

	let split = buffer.clone_for_split(ViewId(1));
	// Split should defer to document (writable), not inherit override
	assert!(!split.is_readonly());
}
