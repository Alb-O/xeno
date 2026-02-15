use nu_protocol::{Record, Span};

use super::*;

#[test]
fn decode_allows_nothing_return() {
	let result = decode_invocations(Value::nothing(Span::unknown())).expect("nothing should decode to empty");
	assert!(result.is_empty());
}

#[test]
fn decode_nothing_in_list() {
	let span = Span::unknown();
	let mut r1 = Record::new();
	r1.push("kind", Value::string("editor", span));
	r1.push("name", Value::string("stats", span));
	let mut r2 = Record::new();
	r2.push("kind", Value::string("action", span));
	r2.push("name", Value::string("move_right", span));
	let list = Value::list(vec![Value::record(r1, span), Value::nothing(span), Value::record(r2, span)], span);
	let result = decode_invocations(list).expect("list with nothing should decode");
	assert_eq!(result.len(), 2);
}

#[test]
fn decode_error_includes_path_for_nested() {
	let span = Span::unknown();
	let mut r1 = Record::new();
	r1.push("kind", Value::string("editor", span));
	r1.push("name", Value::string("stats", span));
	let bad_list = Value::list(vec![Value::record(r1, span), Value::int(42, span)], span);
	let err = decode_invocations(bad_list).expect_err("bad list item should fail");
	assert!(err.contains("return[1]"), "error should include path, got: {err}");
}

#[test]
fn decode_error_includes_path_for_record_field() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("action", span));
	record.push("name", Value::int(42, span));
	let err = decode_invocations(Value::record(record, span)).expect_err("bad field should fail");
	assert!(err.contains("return.name"), "error should include field path, got: {err}");
}

#[test]
fn decode_rejects_wrapper_record() {
	let span = Span::unknown();
	let mut r1 = Record::new();
	r1.push("kind", Value::string("editor", span));
	r1.push("name", Value::string("stats", span));
	let mut wrapper = Record::new();
	wrapper.push("invocations", Value::list(vec![Value::record(r1, span)], span));
	let err = decode_invocations(Value::record(wrapper, span)).expect_err("wrapper should be rejected");
	assert!(err.contains("must include 'kind'"), "error should require kind, got: {err}");
}

#[test]
fn decode_limits_max_nodes_trips_on_large_list() {
	let span = Span::unknown();
	let items: Vec<Value> = (0..200).map(|_| Value::nothing(span)).collect();
	let value = Value::list(items, span);

	let limits = DecodeLimits {
		max_nodes: 100,
		..DecodeLimits::macro_defaults()
	};
	let err = decode_invocations_with_limits(value, limits).expect_err("should trip max_nodes");
	assert!(err.contains("traversal exceeds 100"), "error should mention max_nodes, got: {err}");
}

#[test]
fn decode_action_record_with_null_optionals() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("action", span));
	record.push("name", Value::string("move_right", span));
	record.push("count", Value::int(3, span));
	record.push("extend", Value::bool(false, span));
	record.push("register", Value::nothing(span));
	record.push("char", Value::nothing(span));
	let result = decode_invocations(Value::record(record, span)).expect("null optionals should decode");
	assert!(matches!(
		result.as_slice(),
		[Invocation::Action { name, count: 3, extend: false, register: None }] if name == "move_right"
	));
}

#[test]
fn decode_action_record_with_null_count() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("action", span));
	record.push("name", Value::string("move_right", span));
	record.push("count", Value::nothing(span));
	let result = decode_invocations(Value::record(record, span)).expect("null count should decode");
	assert!(matches!(result.as_slice(), [Invocation::Action { count: 1, .. }]));
}

#[test]
fn decode_command_record_with_null_args() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("command", span));
	record.push("name", Value::string("write", span));
	record.push("args", Value::nothing(span));
	let result = decode_invocations(Value::record(record, span)).expect("null args should decode");
	assert!(matches!(
		result.as_slice(),
		[Invocation::Command { name, args }] if name == "write" && args.is_empty()
	));
}

#[test]
fn decode_rejects_string_return() {
	let span = Span::unknown();
	let err = decode_invocations(Value::string("action:move_right", span)).expect_err("string should be rejected");
	assert!(
		err.contains("string returns are no longer supported"),
		"error should mention string rejection, got: {err}"
	);
	assert!(err.contains("prelude constructors"), "error should suggest constructors, got: {err}");
}

#[test]
fn decode_rejects_string_in_list() {
	let span = Span::unknown();
	let list = Value::list(vec![Value::string("action:move_right", span)], span);
	let err = decode_invocations(list).expect_err("string in list should be rejected");
	assert!(err.contains("return[0]"), "error should include path, got: {err}");
	assert!(err.contains("string returns are no longer supported"), "got: {err}");
}

#[test]
fn decode_defaults_match_docs() {
	let m = DecodeLimits::macro_defaults();
	assert_eq!(m.max_invocations, 256);
	assert_eq!(m.max_args, 64);
	assert_eq!(m.max_depth, 8);
	assert_eq!(m.max_string_len, 4096);
	assert_eq!(m.max_nodes, 50_000);

	let h = DecodeLimits::hook_defaults();
	assert_eq!(h.max_invocations, 32);
	assert_eq!(h.max_args, 64);
	assert_eq!(h.max_nodes, 5_000);
}

#[test]
fn decode_single_invocation_accepts_record() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("command", span));
	record.push("name", Value::string("write", span));
	let inv = decode_single_invocation(&Value::record(record, span), "keys.normal.ctrl+s").expect("should decode");
	assert!(matches!(inv, Invocation::Command { name, .. } if name == "write"));
}

#[test]
fn decode_single_invocation_rejects_string() {
	let span = Span::unknown();
	let err = decode_single_invocation(&Value::string("command:write", span), "keys.normal.ctrl+s").expect_err("string should be rejected");
	assert!(err.contains("expected invocation record"), "got: {err}");
	assert!(err.starts_with("Nu decode error at keys.normal.ctrl+s"), "got: {err}");
}

#[test]
fn decode_single_invocation_rejects_missing_kind() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("name", Value::string("write", span));
	let err = decode_single_invocation(&Value::record(record, span), "keys.normal.ctrl+s").expect_err("missing kind should be rejected");
	assert!(err.contains("keys.normal.ctrl+s"), "error should include field path, got: {err}");
	assert!(err.contains("kind"), "error should mention 'kind', got: {err}");
}

#[test]
fn decode_single_invocation_validates_limits() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("command", span));
	record.push("name", Value::string("x".repeat(5000), span));
	let err = decode_single_invocation(&Value::record(record, span), "keys.insert.f1").expect_err("oversized name should be rejected");
	assert!(err.contains("keys.insert.f1"), "error should include field path, got: {err}");
}

#[test]
fn decode_single_invocation_error_path_format() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("command", span));
	// Missing required 'name' field — reported at parent level
	let err = decode_single_invocation(&Value::record(record, span), "keys.normal.g").expect_err("missing name should be rejected");
	assert!(
		err.starts_with("Nu decode error at keys.normal.g:"),
		"error path should use root label, got: {err}"
	);
	assert!(err.contains("name"), "error should mention missing field, got: {err}");
}
