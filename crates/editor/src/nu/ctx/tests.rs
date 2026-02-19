use super::*;

fn sample_ctx() -> NuCtx {
	NuCtx {
		kind: "macro".into(),
		function: "test_fn".into(),
		mode: "Normal".into(),
		view: NuCtxView { id: 42 },
		cursor: NuCtxPosition { line: 10, col: 5 },
		selection: NuCtxSelection {
			active: true,
			primary: 0,
			start: NuCtxPosition { line: 10, col: 3 },
			end: NuCtxPosition { line: 10, col: 8 },
			ranges: vec![NuCtxRange {
				anchor: NuCtxPosition { line: 10, col: 3 },
				head: NuCtxPosition { line: 10, col: 8 },
			}],
		},
		buffer: NuCtxBuffer {
			path: Some("/tmp/test.rs".into()),
			file_type: Some("rust".into()),
			readonly: false,
			modified: true,
		},
		text: NuCtxText {
			line: Some("hello world".into()),
			line_truncated: false,
			selection: Some("lo wo".into()),
			selection_truncated: false,
		},
		event: None,
		state: vec![],
	}
}

#[test]
fn ctx_has_required_top_level_fields() {
	let value = sample_ctx().to_value();
	let record = value.as_record().expect("ctx should be a record");

	let required = [
		"schema_version",
		"kind",
		"function",
		"mode",
		"view",
		"cursor",
		"selection",
		"buffer",
		"text",
		"event",
		"state",
	];
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
	assert!(selection.contains("primary"));
	assert!(selection.contains("start"));
	assert!(selection.contains("end"));
	assert!(selection.contains("ranges"));

	let start = selection.get("start").expect("start should exist").as_record().expect("start should be record");
	assert!(start.contains("line"));
	assert!(start.contains("col"));

	let ranges = selection.get("ranges").expect("ranges should exist").as_list().expect("ranges should be list");
	assert_eq!(ranges.len(), 1);
	let r0 = ranges[0].as_record().expect("range should be record");
	assert!(r0.contains("anchor"));
	assert!(r0.contains("head"));
}

#[test]
fn ctx_selection_multi_range() {
	let mut ctx = sample_ctx();
	ctx.selection = NuCtxSelection {
		active: true,
		primary: 1,
		start: NuCtxPosition { line: 5, col: 0 },
		end: NuCtxPosition { line: 5, col: 10 },
		ranges: vec![
			NuCtxRange {
				anchor: NuCtxPosition { line: 2, col: 3 },
				head: NuCtxPosition { line: 2, col: 7 },
			},
			NuCtxRange {
				anchor: NuCtxPosition { line: 5, col: 10 },
				head: NuCtxPosition { line: 5, col: 0 },
			},
		],
	};

	let value = ctx.to_value();
	let record = value.as_record().expect("ctx should be a record");
	let selection = record.get("selection").unwrap().as_record().unwrap();

	assert_eq!(selection.get("primary").unwrap().as_int().unwrap(), 1);
	let ranges = selection.get("ranges").unwrap().as_list().unwrap();
	assert_eq!(ranges.len(), 2);

	let r0 = ranges[0].as_record().unwrap();
	let anchor0 = r0.get("anchor").unwrap().as_record().unwrap();
	assert_eq!(anchor0.get("line").unwrap().as_int().unwrap(), 2);
	assert_eq!(anchor0.get("col").unwrap().as_int().unwrap(), 3);

	let r1 = ranges[1].as_record().unwrap();
	let head1 = r1.get("head").unwrap().as_record().unwrap();
	assert_eq!(head1.get("line").unwrap().as_int().unwrap(), 5);
	assert_eq!(head1.get("col").unwrap().as_int().unwrap(), 0);
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

#[test]
fn ctx_text_has_correct_shape_with_content() {
	let value = sample_ctx().to_value();
	let record = value.as_record().expect("ctx should be a record");
	let text = record.get("text").expect("text should exist").as_record().expect("text should be record");

	assert_eq!(text.get("line").unwrap().as_str().unwrap(), "hello world");
	assert!(!text.get("line_truncated").unwrap().as_bool().unwrap());
	assert_eq!(text.get("selection").unwrap().as_str().unwrap(), "lo wo");
	assert!(!text.get("selection_truncated").unwrap().as_bool().unwrap());
}

#[test]
fn ctx_text_empty_has_nothing_fields() {
	let mut ctx = sample_ctx();
	ctx.text = NuCtxText::empty();

	let value = ctx.to_value();
	let record = value.as_record().expect("ctx should be a record");
	let text = record.get("text").expect("text should exist").as_record().expect("text should be record");

	assert!(text.get("line").unwrap().is_nothing());
	assert!(!text.get("line_truncated").unwrap().as_bool().unwrap());
	assert!(text.get("selection").unwrap().is_nothing());
	assert!(!text.get("selection_truncated").unwrap().as_bool().unwrap());
}

#[test]
fn clamp_utf8_no_truncation() {
	let (s, trunc) = clamp_utf8("hello", 10);
	assert_eq!(s, "hello");
	assert!(!trunc);
}

#[test]
fn clamp_utf8_exact_boundary() {
	let (s, trunc) = clamp_utf8("hello", 5);
	assert_eq!(s, "hello");
	assert!(!trunc);
}

#[test]
fn clamp_utf8_truncates_ascii() {
	let (s, trunc) = clamp_utf8("hello world", 5);
	assert_eq!(s, "hello");
	assert!(trunc);
}

#[test]
fn clamp_utf8_respects_char_boundary() {
	// "é" is 2 bytes in UTF-8; cutting at byte 1 must back up
	let (s, trunc) = clamp_utf8("é", 1);
	assert_eq!(s, "");
	assert!(trunc);
}

#[test]
fn clamp_utf8_multibyte_safe() {
	// "aé" = 3 bytes; cap at 2 must keep only "a"
	let (s, trunc) = clamp_utf8("aé", 2);
	assert_eq!(s, "a");
	assert!(trunc);
}

#[test]
fn rope_slice_clamped_small_fits() {
	use xeno_primitives::Rope;
	let rope = Rope::from("hello world");
	let (s, trunc) = rope_slice_clamped(rope.slice(..), 100);
	assert_eq!(s, "hello world");
	assert!(!trunc);
}

#[test]
fn rope_slice_clamped_truncates_at_budget() {
	use xeno_primitives::Rope;
	let rope = Rope::from("hello world");
	let (s, trunc) = rope_slice_clamped(rope.slice(..), 5);
	assert_eq!(s, "hello");
	assert!(trunc);
}

#[test]
fn rope_slice_clamped_large_ascii() {
	use xeno_primitives::Rope;
	let big = "x".repeat(100_000);
	let rope = Rope::from(big.as_str());
	let (s, trunc) = rope_slice_clamped(rope.slice(..), 64);
	assert_eq!(s.len(), 64);
	assert!(trunc);
}

#[test]
fn rope_slice_clamped_multibyte_boundary() {
	use xeno_primitives::Rope;
	// "aéb" = a(1) + é(2) + b(1) = 4 bytes
	let rope = Rope::from("aéb");
	let (s, trunc) = rope_slice_clamped(rope.slice(..), 2);
	// Can fit "a" (1 byte), "é" starts at byte 1 and needs 2 bytes → only 1 remaining → back up
	assert_eq!(s, "a");
	assert!(trunc);
}

#[test]
fn rope_slice_clamped_zero_budget() {
	use xeno_primitives::Rope;
	let rope = Rope::from("hello");
	let (s, trunc) = rope_slice_clamped(rope.slice(..), 0);
	assert_eq!(s, "");
	assert!(trunc);
}

#[test]
fn rope_slice_clamped_empty_slice() {
	use xeno_primitives::Rope;
	let rope = Rope::from("");
	let (s, trunc) = rope_slice_clamped(rope.slice(..), 100);
	assert_eq!(s, "");
	assert!(!trunc);
}

#[test]
fn ctx_event_null_for_macro() {
	let ctx = sample_ctx();
	let value = ctx.to_value();
	let record = value.as_record().expect("ctx should be a record");
	assert!(record.get("event").unwrap().is_nothing());
}

#[test]
fn ctx_event_action_post_shape() {
	let mut ctx = sample_ctx();
	ctx.event = Some(NuCtxEvent::ActionPost {
		name: "move_right".into(),
		result: "ok".into(),
	});
	let value = ctx.to_value();
	let record = value.as_record().expect("ctx should be a record");
	let event = record.get("event").unwrap().as_record().expect("event should be record");
	assert_eq!(event.get("type").unwrap().as_str().unwrap(), "action_post");
	let data = event.get("data").unwrap().as_record().expect("data should be record");
	assert_eq!(data.get("name").unwrap().as_str().unwrap(), "move_right");
	assert_eq!(data.get("result").unwrap().as_str().unwrap(), "ok");
}

#[test]
fn ctx_event_command_post_includes_args() {
	let mut ctx = sample_ctx();
	ctx.event = Some(NuCtxEvent::CommandPost {
		name: "write".into(),
		result: "ok".into(),
		args: vec!["--force".into()],
	});
	let value = ctx.to_value();
	let record = value.as_record().expect("ctx should be a record");
	let event = record.get("event").unwrap().as_record().expect("event should be record");
	assert_eq!(event.get("type").unwrap().as_str().unwrap(), "command_post");
	let data = event.get("data").unwrap().as_record().expect("data should be record");
	assert_eq!(data.get("name").unwrap().as_str().unwrap(), "write");
	let args = data.get("args").unwrap().as_list().expect("args should be list");
	assert_eq!(args.len(), 1);
	assert_eq!(args[0].as_str().unwrap(), "--force");
}

#[test]
fn ctx_event_mode_change_shape() {
	let mut ctx = sample_ctx();
	ctx.event = Some(NuCtxEvent::ModeChange {
		from: "Normal".into(),
		to: "Insert".into(),
	});
	let value = ctx.to_value();
	let record = value.as_record().expect("ctx should be a record");
	let event = record.get("event").unwrap().as_record().expect("event should be record");
	assert_eq!(event.get("type").unwrap().as_str().unwrap(), "mode_change");
	let data = event.get("data").unwrap().as_record().expect("data should be record");
	assert_eq!(data.get("from").unwrap().as_str().unwrap(), "Normal");
	assert_eq!(data.get("to").unwrap().as_str().unwrap(), "Insert");
}

#[test]
fn ctx_no_args_field() {
	let value = sample_ctx().to_value();
	let record = value.as_record().expect("ctx should be a record");
	assert!(!record.contains("args"), "args field should not exist in ctx schema v7");
}

#[test]
fn ctx_state_is_record_map() {
	let mut ctx = sample_ctx();
	ctx.state = vec![("debounce".into(), "123".into()), ("last_path".into(), "/tmp/a".into())];
	let value = ctx.to_value();
	let record = value.as_record().expect("ctx should be a record");
	let state = record.get("state").expect("state should exist").as_record().expect("state should be record");
	assert_eq!(state.len(), 2);
	assert_eq!(state.get("debounce").unwrap().as_str().unwrap(), "123");
	assert_eq!(state.get("last_path").unwrap().as_str().unwrap(), "/tmp/a");
}

#[test]
fn ctx_state_empty_is_empty_record() {
	let value = sample_ctx().to_value();
	let record = value.as_record().expect("ctx should be a record");
	let state = record.get("state").expect("state should exist").as_record().expect("state should be record");
	assert!(state.is_empty());
}
