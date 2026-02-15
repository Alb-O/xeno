use super::*;

fn sample_ctx() -> NuCtx {
	NuCtx {
		kind: "macro".into(),
		function: "test_fn".into(),
		args: vec!["a".into(), "b".into()],
		mode: "Normal".into(),
		view: NuCtxView { id: 42 },
		cursor: NuCtxPosition { line: 10, col: 5 },
		selection: NuCtxSelection {
			active: true,
			start: NuCtxPosition { line: 10, col: 3 },
			end: NuCtxPosition { line: 10, col: 8 },
		},
		buffer: NuCtxBuffer {
			path: Some("/tmp/test.rs".into()),
			file_type: Some("rust".into()),
			readonly: false,
			modified: true,
		},
	}
}

#[test]
fn ctx_has_required_top_level_fields() {
	let value = sample_ctx().to_value();
	let record = value.as_record().expect("ctx should be a record");

	let required = ["schema_version", "kind", "function", "args", "mode", "view", "cursor", "selection", "buffer"];
	for field in required {
		assert!(record.contains(field), "missing required field: {field}");
	}
}

#[test]
fn ctx_schema_version_is_current() {
	let value = sample_ctx().to_value();
	let record = value.as_record().expect("ctx should be a record");
	let version = record.get("schema_version").expect("schema_version should exist");
	assert_eq!(version.as_int().expect("should be int"), SCHEMA_VERSION);
}

#[test]
fn ctx_buffer_path_is_nothing_when_absent() {
	let mut ctx = sample_ctx();
	ctx.buffer.path = None;
	ctx.buffer.file_type = None;

	let value = ctx.to_value();
	let record = value.as_record().expect("ctx should be a record");
	let buffer = record.get("buffer").expect("buffer should exist").as_record().expect("buffer should be record");
	assert!(buffer.get("path").expect("path should exist").is_nothing());
	assert!(buffer.get("file_type").expect("file_type should exist").is_nothing());
}

#[test]
fn ctx_selection_has_correct_shape() {
	let value = sample_ctx().to_value();
	let record = value.as_record().expect("ctx should be a record");
	let selection = record
		.get("selection")
		.expect("selection should exist")
		.as_record()
		.expect("selection should be record");

	assert!(selection.contains("active"));
	assert!(selection.contains("start"));
	assert!(selection.contains("end"));

	let start = selection.get("start").expect("start should exist").as_record().expect("start should be record");
	assert!(start.contains("line"));
	assert!(start.contains("col"));
}

#[test]
fn ctx_cursor_has_line_and_col() {
	let value = sample_ctx().to_value();
	let record = value.as_record().expect("ctx should be a record");
	let cursor = record.get("cursor").expect("cursor should exist").as_record().expect("cursor should be record");
	assert!(cursor.contains("line"));
	assert!(cursor.contains("col"));
	assert_eq!(cursor.get("line").unwrap().as_int().unwrap(), 10);
	assert_eq!(cursor.get("col").unwrap().as_int().unwrap(), 5);
}
