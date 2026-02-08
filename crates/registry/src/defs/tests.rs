use crate::defs::link::link_by_name;
use crate::defs::loader::{MAGIC, SCHEMA_VERSION, validate_blob};

#[test]
fn test_validate_blob_valid() {
	let mut data = Vec::new();
	data.extend_from_slice(MAGIC);
	data.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
	data.extend_from_slice(b"payload");
	assert_eq!(validate_blob(&data), Some(b"payload".as_slice()));
}

#[test]
fn test_validate_blob_invalid_magic() {
	let mut data = Vec::new();
	data.extend_from_slice(b"BADMAGIC");
	data.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
	data.extend_from_slice(b"payload");
	assert_eq!(validate_blob(&data), None);
}

#[test]
fn test_validate_blob_invalid_version() {
	let mut data = Vec::new();
	data.extend_from_slice(MAGIC);
	data.extend_from_slice(&(SCHEMA_VERSION + 1).to_le_bytes());
	data.extend_from_slice(b"payload");
	assert_eq!(validate_blob(&data), None);
}

#[test]
fn test_validate_blob_too_short() {
	let data = b"short";
	assert_eq!(validate_blob(data), None);
}

#[test]
#[should_panic(expected = "duplicate")]
fn test_link_by_name_duplicate_handler() {
	struct Meta {
		name: String,
	}
	struct Handler {
		name: &'static str,
	}

	let metas = vec![Meta { name: "foo".into() }];
	let handlers = vec![
		Handler { name: "foo" },
		Handler { name: "foo" }, // Duplicate
	];
	let leaked_handlers: &'static [Handler] = Box::leak(handlers.into_boxed_slice());

	link_by_name(
		&metas,
		leaked_handlers.iter(),
		|m| &m.name,
		|h| h.name,
		|_, _| (),
		"test",
	);
}

#[test]
#[should_panic(expected = "has no matching")]
fn test_link_by_name_missing_handler() {
	struct Meta {
		name: String,
	}
	struct Handler {
		name: &'static str,
	}

	let metas = vec![Meta { name: "foo".into() }];
	let handlers = vec![Handler { name: "bar" }];
	let leaked_handlers: &'static [Handler] = Box::leak(handlers.into_boxed_slice());

	link_by_name(
		&metas,
		leaked_handlers.iter(),
		|m| &m.name,
		|h| h.name,
		|_, _| (),
		"test",
	);
}

#[test]
#[should_panic(expected = "has no matching entry in spec")]
fn test_link_by_name_unused_handler() {
	struct Meta {
		name: String,
	}
	struct Handler {
		name: &'static str,
	}

	let metas = vec![Meta { name: "foo".into() }];
	let handlers = vec![
		Handler { name: "foo" },
		Handler { name: "bar" }, // Unused
	];
	let leaked_handlers: &'static [Handler] = Box::leak(handlers.into_boxed_slice());

	link_by_name(
		&metas,
		leaked_handlers.iter(),
		|m| &m.name,
		|h| h.name,
		|_, _| (),
		"test",
	);
}
