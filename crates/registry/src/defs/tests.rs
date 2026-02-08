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
#[should_panic(expected = "duplicate handlers (1)")]
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
#[should_panic(expected = "spec entries missing handlers (1)")]
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
#[should_panic(expected = "handlers missing spec entries (1)")]
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

#[test]
fn test_link_by_name_aggregate_report() {
	struct Meta {
		name: String,
	}
	struct Handler {
		name: &'static str,
	}

	let metas = vec![
		Meta {
			name: "missing".into(),
		},
		Meta {
			name: "dup_meta".into(),
		},
		Meta {
			name: "dup_meta".into(),
		},
	];
	let handlers = vec![
		Handler { name: "extra" },
		Handler {
			name: "dup_handler",
		},
		Handler {
			name: "dup_handler",
		},
	];
	let leaked_handlers: &'static [Handler] = Box::leak(handlers.into_boxed_slice());

	let result = std::panic::catch_unwind(|| {
		link_by_name(
			&metas,
			leaked_handlers.iter(),
			|m| &m.name,
			|h| h.name,
			|_, _| (),
			"test",
		);
	});

	let err = result.expect_err("should have panicked");
	let msg = err
		.downcast_ref::<String>()
		.expect("panic msg should be String");

	assert!(msg.contains("link_by_name(test) failed:"));
	assert!(msg.contains("duplicate handlers (1):"));
	assert!(msg.contains("- dup_handler"));
	assert!(msg.contains("duplicate spec entries (1):"));
	assert!(msg.contains("- dup_meta"));
	assert!(msg.contains("spec entries missing handlers (2):"));
	assert!(msg.contains("- missing"));
	assert!(msg.contains("handlers missing spec entries (2):"));
	assert!(msg.contains("- extra"));
}
