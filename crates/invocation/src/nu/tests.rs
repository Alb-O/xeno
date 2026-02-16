use xeno_nu_value::{CustomValue, Record, Span, Value};

use super::*;

// ---------------------------------------------------------------------------
// Runtime decode (typed-only)
// ---------------------------------------------------------------------------

#[test]
fn runtime_allows_nothing_return() {
	let result = decode_invocations(Value::nothing(Span::unknown())).expect("nothing should decode to empty");
	assert!(result.is_empty());
}

#[test]
fn runtime_decodes_custom_value() {
	let span = Span::unknown();
	let inv = Invocation::action("move_right");
	let value = InvocationValue(inv.clone()).into_value(span);
	let result = decode_invocations(value).expect("custom value should decode");
	assert_eq!(result, vec![inv]);
}

#[test]
fn runtime_decodes_list_of_custom_values() {
	let span = Span::unknown();
	let inv1 = Invocation::action("move_right");
	let inv2 = Invocation::command("write", vec![]);
	let list = Value::list(
		vec![
			InvocationValue(inv1.clone()).into_value(span),
			Value::nothing(span),
			InvocationValue(inv2.clone()).into_value(span),
		],
		span,
	);
	let result = decode_invocations(list).expect("list of custom values should decode");
	assert_eq!(result, vec![inv1, inv2]);
}

#[test]
fn runtime_rejects_record_return() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("action", span));
	record.push("name", Value::string("move_right", span));
	let err = decode_invocations(Value::record(record, span)).expect_err("record should be rejected");
	assert!(err.contains("record returns are not supported at runtime"), "got: {err}");
}

#[test]
fn runtime_rejects_record_in_list() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("action", span));
	record.push("name", Value::string("move_right", span));
	let list = Value::list(vec![Value::record(record, span)], span);
	let err = decode_invocations(list).expect_err("record in list should be rejected");
	assert!(err.contains("return[0]"), "error should include path, got: {err}");
	assert!(err.contains("record returns are not supported at runtime"), "got: {err}");
}

#[test]
fn runtime_rejects_nested_list() {
	let span = Span::unknown();
	let inner = Value::list(vec![Value::nothing(span)], span);
	let outer = Value::list(vec![inner], span);
	let err = decode_invocations(outer).expect_err("nested list should be rejected");
	assert!(err.contains("return[0]"), "error should include path, got: {err}");
}

#[test]
fn runtime_rejects_string_return() {
	let span = Span::unknown();
	let err = decode_invocations(Value::string("action:move_right", span)).expect_err("string should be rejected");
	assert!(err.contains("string returns are not supported"), "got: {err}");
}

#[test]
fn runtime_rejects_string_in_list() {
	let span = Span::unknown();
	let list = Value::list(vec![Value::string("action:move_right", span)], span);
	let err = decode_invocations(list).expect_err("string in list should be rejected");
	assert!(err.contains("return[0]"), "error should include path, got: {err}");
}

#[test]
fn runtime_error_includes_path_for_bad_item() {
	let span = Span::unknown();
	let inv = Invocation::action("move_right");
	let list = Value::list(vec![InvocationValue(inv).into_value(span), Value::int(42, span)], span);
	let err = decode_invocations(list).expect_err("bad list item should fail");
	assert!(err.contains("return[1]"), "error should include path, got: {err}");
}

#[test]
fn runtime_limits_max_nodes_trips_on_large_list() {
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
fn runtime_custom_value_enforces_limits() {
	let span = Span::unknown();
	let inv = Invocation::Action {
		name: "x".repeat(5000),
		count: 1,
		extend: false,
		register: None,
	};
	let value = InvocationValue(inv).into_value(span);
	let err = decode_invocations(value).expect_err("oversized name in custom value should be rejected");
	assert!(err.contains("max string length"), "got: {err}");
}

// ---------------------------------------------------------------------------
// Config decode (decode_single_invocation — records + custom)
// ---------------------------------------------------------------------------

#[test]
fn config_single_accepts_record() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("command", span));
	record.push("name", Value::string("write", span));
	let inv = decode_single_invocation(&Value::record(record, span), "keys.normal.ctrl+s").expect("should decode");
	assert!(matches!(inv, Invocation::Command { name, .. } if name == "write"));
}

#[test]
fn config_single_accepts_custom_value() {
	let span = Span::unknown();
	let inv = Invocation::editor_command("stats", vec![]);
	let value = InvocationValue(inv.clone()).into_value(span);
	let result = decode_single_invocation(&value, "keys.normal.s").expect("should decode custom value");
	assert_eq!(result, inv);
}

#[test]
fn config_single_rejects_string() {
	let span = Span::unknown();
	let err = decode_single_invocation(&Value::string("command:write", span), "keys.normal.ctrl+s").expect_err("string should be rejected");
	assert!(err.contains("expected invocation record"), "got: {err}");
	assert!(err.starts_with("Nu decode error at keys.normal.ctrl+s"), "got: {err}");
}

#[test]
fn config_single_rejects_missing_kind() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("name", Value::string("write", span));
	let err = decode_single_invocation(&Value::record(record, span), "keys.normal.ctrl+s").expect_err("missing kind should be rejected");
	assert!(err.contains("keys.normal.ctrl+s"), "error should include field path, got: {err}");
	assert!(err.contains("kind"), "error should mention 'kind', got: {err}");
}

#[test]
fn config_single_validates_limits() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("command", span));
	record.push("name", Value::string("x".repeat(5000), span));
	let err = decode_single_invocation(&Value::record(record, span), "keys.insert.f1").expect_err("oversized name should be rejected");
	assert!(err.contains("keys.insert.f1"), "error should include field path, got: {err}");
}

#[test]
fn config_single_error_path_format() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("command", span));
	let err = decode_single_invocation(&Value::record(record, span), "keys.normal.g").expect_err("missing name should be rejected");
	assert!(
		err.starts_with("Nu decode error at keys.normal.g:"),
		"error path should use root label, got: {err}"
	);
	assert!(err.contains("name"), "error should mention missing field, got: {err}");
}

#[test]
fn config_action_record_with_null_optionals() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("action", span));
	record.push("name", Value::string("move_right", span));
	record.push("count", Value::int(3, span));
	record.push("extend", Value::bool(false, span));
	record.push("register", Value::nothing(span));
	record.push("char", Value::nothing(span));
	let inv = decode_single_invocation(&Value::record(record, span), "keys.normal.l").expect("null optionals should decode");
	assert!(matches!(inv, Invocation::Action { name, count: 3, extend: false, register: None } if name == "move_right"));
}

#[test]
fn config_action_record_with_null_count() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("action", span));
	record.push("name", Value::string("move_right", span));
	record.push("count", Value::nothing(span));
	let inv = decode_single_invocation(&Value::record(record, span), "keys.normal.l").expect("null count should decode");
	assert!(matches!(inv, Invocation::Action { count: 1, .. }));
}

#[test]
fn config_command_record_with_null_args() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("kind", Value::string("command", span));
	record.push("name", Value::string("write", span));
	record.push("args", Value::nothing(span));
	let inv = decode_single_invocation(&Value::record(record, span), "keys.normal.w").expect("null args should decode");
	assert!(matches!(inv, Invocation::Command { name, args } if name == "write" && args.is_empty()));
}

#[test]
fn config_rejects_wrapper_record() {
	let span = Span::unknown();
	let mut r1 = Record::new();
	r1.push("kind", Value::string("editor", span));
	r1.push("name", Value::string("stats", span));
	let mut wrapper = Record::new();
	wrapper.push("invocations", Value::list(vec![Value::record(r1, span)], span));
	let err = decode_single_invocation(&Value::record(wrapper, span), "keys.normal.s").expect_err("wrapper should be rejected");
	assert!(err.contains("must include 'kind'"), "error should require kind, got: {err}");
}

#[test]
fn config_single_custom_value_enforces_limits() {
	let span = Span::unknown();
	let inv = Invocation::Command {
		name: "x".repeat(5000),
		args: vec![],
	};
	let value = InvocationValue(inv).into_value(span);
	let err = decode_single_invocation(&value, "keys.normal.x").expect_err("oversized name should be rejected");
	assert!(err.contains("keys.normal.x"), "error should include field path, got: {err}");
}

// ---------------------------------------------------------------------------
// CustomValue / InvocationValue
// ---------------------------------------------------------------------------

#[test]
fn custom_value_to_base_value_roundtrips() {
	let span = Span::unknown();
	let inv = Invocation::Action {
		name: "move_right".to_string(),
		count: 3,
		extend: true,
		register: Some('a'),
	};
	let cv = InvocationValue(inv);
	let base = cv.to_base_value(span).expect("to_base_value should succeed");
	let record = base.as_record().expect("should be record");
	assert_eq!(record.get("kind").unwrap().as_str().unwrap(), "action");
	assert_eq!(record.get("name").unwrap().as_str().unwrap(), "move_right");
	assert_eq!(record.get("count").unwrap().as_int().unwrap(), 3);
}

// ---------------------------------------------------------------------------
// Limits defaults
// ---------------------------------------------------------------------------

#[test]
fn decode_defaults_match_docs() {
	let m = DecodeLimits::macro_defaults();
	assert_eq!(m.max_invocations, 256);
	assert_eq!(m.max_args, 64);
	assert_eq!(m.max_string_len, 4096);
	assert_eq!(m.max_nodes, 50_000);

	let h = DecodeLimits::hook_defaults();
	assert_eq!(h.max_invocations, 32);
	assert_eq!(h.max_args, 64);
	assert_eq!(h.max_nodes, 5_000);
}
