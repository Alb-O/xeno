use xeno_nu_data::{Record, Span, Value};

use super::*;

fn dispatch_record(span: Span, kind: &str, name: &str) -> Value {
	let mut r = Record::new();
	r.push("type", Value::string("dispatch", span));
	r.push("kind", Value::string(kind, span));
	r.push("name", Value::string(name, span));
	Value::record(r, span)
}

/// Wrap one or more effect records in an envelope for the decoder.
fn envelope(span: Span, effects: Vec<Value>) -> Value {
	let mut r = Record::new();
	r.push("schema_version", Value::int(1, span));
	r.push("effects", Value::list(effects, span));
	Value::record(r, span)
}

fn envelope1(span: Span, effect: Value) -> Value {
	envelope(span, vec![effect])
}

#[test]
fn macro_decode_allows_nothing_return() {
	let decoded = decode_macro_effects(Value::nothing(Span::unknown())).expect("nothing should decode");
	assert!(decoded.effects.is_empty());
}

#[test]
fn macro_decode_accepts_dispatch_envelope() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, dispatch_record(span, "action", "move_right"))).expect("dispatch should decode");
	assert_eq!(decoded.schema_version, 1);
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(decoded.effects[0], NuEffect::Dispatch(Invocation::Action { ref name, .. }) if name == "move_right"));
}

#[test]
fn macro_decode_rejects_stop_effect() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("stop", span));
	let err = decode_macro_effects(envelope1(span, Value::record(r, span))).expect_err("macro stop should fail");
	assert!(err.contains("only allowed in hook"), "got: {err}");
}

#[test]
fn hook_decode_accepts_stop_effect() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("stop", span));
	let decoded = decode_hook_effects(envelope1(span, Value::record(r, span))).expect("hook stop should decode");
	assert!(decoded.has_stop_propagation());
}

#[test]
fn macro_decode_accepts_bare_effect_record() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(dispatch_record(span, "action", "move_right")).expect("bare record should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(decoded.effects[0], NuEffect::Dispatch(Invocation::Action { ref name, .. }) if name == "move_right"));
}

#[test]
fn macro_decode_accepts_bare_list() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(Value::list(vec![dispatch_record(span, "action", "move_right")], span)).expect("bare list should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(decoded.effects[0], NuEffect::Dispatch(Invocation::Action { ref name, .. }) if name == "move_right"));
}

#[test]
fn macro_decode_bare_record_rejects_stop() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("stop", span));
	let err = decode_macro_effects(Value::record(r, span)).expect_err("bare stop on macro surface should fail");
	assert!(err.contains("only allowed in hook"), "got: {err}");
}

#[test]
fn decode_envelope_list() {
	let span = Span::unknown();
	let mut envelope = Record::new();
	envelope.push("schema_version", Value::int(EFFECT_SCHEMA_VERSION, span));
	envelope.push("effects", Value::list(vec![dispatch_record(span, "command", "write")], span));
	let decoded = decode_macro_effects(Value::record(envelope, span)).expect("envelope should decode");
	assert_eq!(decoded.schema_version, EFFECT_SCHEMA_VERSION);
	assert!(matches!(
		decoded.effects[0],
		NuEffect::Dispatch(Invocation::Command(crate::CommandInvocation { ref name, .. })) if name == "write"
	));
}

#[test]
fn decode_rejects_legacy_invocation_record() {
	let span = Span::unknown();
	let mut legacy = Record::new();
	legacy.push("kind", Value::string("action", span));
	legacy.push("name", Value::string("move_right", span));
	let err = decode_macro_effects(Value::record(legacy, span)).expect_err("legacy record should fail");
	assert!(err.contains("legacy invocation records"), "got: {err}");
}

#[test]
fn decode_rejects_legacy_invocation_in_envelope() {
	let span = Span::unknown();
	let mut legacy = Record::new();
	legacy.push("kind", Value::string("action", span));
	legacy.push("name", Value::string("move_right", span));
	let err = decode_macro_effects(envelope1(span, Value::record(legacy, span))).expect_err("legacy in envelope should fail");
	assert!(err.contains("legacy invocation records"), "got: {err}");
}

#[test]
fn config_single_dispatch_effect_accepts_record() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("type", Value::string("dispatch", span));
	record.push("kind", Value::string("command", span));
	record.push("name", Value::string("write", span));
	let inv = decode_single_dispatch_effect(&Value::record(record, span), "keys.normal.ctrl+s").expect("should decode");
	assert!(matches!(
		inv,
		Invocation::Command(crate::CommandInvocation { name, .. }) if name == "write"
	));
}

#[test]
fn config_single_dispatch_effect_rejects_notify() {
	let span = Span::unknown();
	let mut record = Record::new();
	record.push("type", Value::string("notify", span));
	record.push("level", Value::string("warn", span));
	record.push("message", Value::string("nope", span));
	let err = decode_single_dispatch_effect(&Value::record(record, span), "keys.normal.ctrl+s").expect_err("notify should fail");
	assert!(err.contains("expected dispatch effect"), "got: {err}");
}

#[test]
fn permission_for_dispatch_matches_invocation_kind() {
	assert_eq!(
		required_permission_for_effect(&NuEffect::Dispatch(Invocation::action("move_right"))),
		NuPermission::DispatchAction
	);
	assert_eq!(
		required_permission_for_effect(&NuEffect::Dispatch(Invocation::command("write", vec![]))),
		NuPermission::DispatchCommand
	);
	assert_eq!(
		required_permission_for_effect(&NuEffect::Notify {
			level: NuNotifyLevel::Warn,
			message: "warn".to_string()
		}),
		NuPermission::Notify
	);
}

#[test]
fn decode_defaults_match_docs() {
	let m = DecodeBudget::macro_defaults();
	assert_eq!(m.max_effects, 256);
	assert_eq!(m.max_args, 64);
	assert_eq!(m.max_string_len, 4096);
	assert_eq!(m.max_nodes, 50_000);

	let h = DecodeBudget::hook_defaults();
	assert_eq!(h.max_effects, 32);
	assert_eq!(h.max_args, 64);
	assert_eq!(h.max_nodes, 5_000);
}

fn edit_record(span: Span, op: &str, text: &str) -> Value {
	let mut r = Record::new();
	r.push("type", Value::string("edit", span));
	r.push("op", Value::string(op, span));
	r.push("text", Value::string(text, span));
	Value::record(r, span)
}

#[test]
fn decode_edit_replace_selection() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, edit_record(span, "replace_selection", "HELLO"))).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::EditText { op: NuTextEditOp::ReplaceSelection, text } if text == "HELLO"
	));
}

#[test]
fn decode_edit_replace_line() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, edit_record(span, "replace_line", "new content"))).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::EditText { op: NuTextEditOp::ReplaceLine, text } if text == "new content"
	));
}

#[test]
fn decode_edit_replace_line_rejects_newline() {
	let span = Span::unknown();
	let err = decode_macro_effects(envelope1(span, edit_record(span, "replace_line", "line1\nline2"))).expect_err("newline should fail");
	assert!(err.contains("newline"), "got: {err}");
}

#[test]
fn decode_edit_replace_selection_allows_empty_text() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, edit_record(span, "replace_selection", ""))).expect("empty text should decode");
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::EditText { op: NuTextEditOp::ReplaceSelection, text } if text.is_empty()
	));
}

#[test]
fn decode_edit_unknown_op_errors() {
	let span = Span::unknown();
	let err = decode_macro_effects(envelope1(span, edit_record(span, "unknown_op", "text"))).expect_err("unknown op should fail");
	assert!(err.contains("unknown edit op"), "got: {err}");
}

#[test]
fn permission_for_edit_text() {
	assert_eq!(
		required_permission_for_effect(&NuEffect::EditText {
			op: NuTextEditOp::ReplaceSelection,
			text: "x".into()
		}),
		NuPermission::EditText
	);
}

fn clipboard_record(span: Span, text: &str) -> Value {
	let mut r = Record::new();
	r.push("type", Value::string("clipboard", span));
	r.push("text", Value::string(text, span));
	Value::record(r, span)
}

#[test]
fn decode_clipboard_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, clipboard_record(span, "copied text"))).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::SetClipboard { text } if text == "copied text"
	));
}

#[test]
fn decode_clipboard_empty_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, clipboard_record(span, ""))).expect("empty clipboard should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::SetClipboard { text } if text.is_empty()
	));
}

#[test]
fn permission_for_set_clipboard() {
	assert_eq!(
		required_permission_for_effect(&NuEffect::SetClipboard { text: "x".into() }),
		NuPermission::SetClipboard
	);
}

fn state_set_record(span: Span, key: &str, value: &str) -> Value {
	let mut r = Record::new();
	r.push("type", Value::string("state", span));
	r.push("op", Value::string("set", span));
	r.push("key", Value::string(key, span));
	r.push("value", Value::string(value, span));
	Value::record(r, span)
}

fn state_unset_record(span: Span, key: &str) -> Value {
	let mut r = Record::new();
	r.push("type", Value::string("state", span));
	r.push("op", Value::string("unset", span));
	r.push("key", Value::string(key, span));
	Value::record(r, span)
}

#[test]
fn decode_state_set_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, state_set_record(span, "foo", "bar"))).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::StateSet { key, value } if key == "foo" && value == "bar"
	));
}

#[test]
fn decode_state_set_empty_value_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, state_set_record(span, "foo", ""))).expect("empty value should decode");
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::StateSet { key, value } if key == "foo" && value.is_empty()
	));
}

#[test]
fn decode_state_unset_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, state_unset_record(span, "foo"))).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::StateUnset { key } if key == "foo"
	));
}

#[test]
fn decode_state_bad_op_errors() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("state", span));
	r.push("op", Value::string("delete", span));
	r.push("key", Value::string("foo", span));
	let err = decode_macro_effects(envelope1(span, Value::record(r, span))).expect_err("bad op should fail");
	assert!(err.contains("unknown state op"), "got: {err}");
}

#[test]
fn decode_state_empty_key_errors() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("state", span));
	r.push("op", Value::string("set", span));
	r.push("key", Value::string("", span));
	r.push("value", Value::string("bar", span));
	let err = decode_macro_effects(envelope1(span, Value::record(r, span))).expect_err("empty key should fail");
	assert!(err.contains("must not be empty"), "got: {err}");
}

#[test]
fn permission_for_write_state() {
	assert_eq!(
		required_permission_for_effect(&NuEffect::StateSet {
			key: "k".into(),
			value: "v".into()
		}),
		NuPermission::WriteState
	);
	assert_eq!(
		required_permission_for_effect(&NuEffect::StateUnset { key: "k".into() }),
		NuPermission::WriteState
	);
}

fn schedule_set_record(span: Span, key: &str, delay_ms: i64, macro_name: &str, args: Vec<&str>) -> Value {
	let mut r = Record::new();
	r.push("type", Value::string("schedule", span));
	r.push("op", Value::string("set", span));
	r.push("key", Value::string(key, span));
	r.push("delay_ms", Value::int(delay_ms, span));
	r.push("macro", Value::string(macro_name, span));
	r.push("args", Value::list(args.iter().map(|a| Value::string(*a, span)).collect(), span));
	Value::record(r, span)
}

fn schedule_cancel_record(span: Span, key: &str) -> Value {
	let mut r = Record::new();
	r.push("type", Value::string("schedule", span));
	r.push("op", Value::string("cancel", span));
	r.push("key", Value::string(key, span));
	Value::record(r, span)
}

#[test]
fn decode_schedule_set_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, schedule_set_record(span, "autosave", 750, "save-all", vec![]))).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::ScheduleSet { key, delay_ms, name, args }
			if key == "autosave" && *delay_ms == 750 && name == "save-all" && args.is_empty()
	));
}

#[test]
fn decode_schedule_set_with_args() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, schedule_set_record(span, "fmt", 300, "format-buffer", vec!["--quiet"]))).expect("should decode");
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::ScheduleSet { key, name, args, .. }
			if key == "fmt" && name == "format-buffer" && args == &["--quiet"]
	));
}

#[test]
fn decode_schedule_cancel_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(envelope1(span, schedule_cancel_record(span, "autosave"))).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::ScheduleCancel { key } if key == "autosave"
	));
}

#[test]
fn decode_schedule_bad_delay_errors() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("schedule", span));
	r.push("op", Value::string("set", span));
	r.push("key", Value::string("k", span));
	r.push("delay_ms", Value::int(3_600_001, span));
	r.push("macro", Value::string("m", span));
	let err = decode_macro_effects(envelope1(span, Value::record(r, span))).expect_err("excessive delay should fail");
	assert!(err.contains("exceeds max"), "got: {err}");
}

#[test]
fn decode_schedule_bad_op_errors() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("schedule", span));
	r.push("op", Value::string("pause", span));
	r.push("key", Value::string("k", span));
	let err = decode_macro_effects(envelope1(span, Value::record(r, span))).expect_err("bad op should fail");
	assert!(err.contains("unknown schedule op"), "got: {err}");
}

#[test]
fn decode_schedule_empty_key_errors() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("schedule", span));
	r.push("op", Value::string("set", span));
	r.push("key", Value::string("", span));
	r.push("delay_ms", Value::int(100, span));
	r.push("macro", Value::string("m", span));
	let err = decode_macro_effects(envelope1(span, Value::record(r, span))).expect_err("empty key should fail");
	assert!(err.contains("must not be empty"), "got: {err}");
}

#[test]
fn decode_envelope_with_warnings() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("schema_version", Value::int(1, span));
	r.push("effects", Value::list(vec![dispatch_record(span, "action", "move_right")], span));
	r.push("warnings", Value::list(vec![Value::string("heads up", span)], span));
	let decoded = decode_macro_effects(Value::record(r, span)).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert_eq!(decoded.warnings, vec!["heads up"]);
}

#[test]
fn permission_for_schedule_macro() {
	assert_eq!(
		required_permission_for_effect(&NuEffect::ScheduleSet {
			key: "k".into(),
			delay_ms: 100,
			name: "m".into(),
			args: vec![]
		}),
		NuPermission::ScheduleMacro
	);
	assert_eq!(
		required_permission_for_effect(&NuEffect::ScheduleCancel { key: "k".into() }),
		NuPermission::ScheduleMacro
	);
}

#[test]
fn decode_rejects_future_schema_version() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("schema_version", Value::int(EFFECT_SCHEMA_VERSION + 1, span));
	r.push("effects", Value::list(vec![], span));
	let err = decode_macro_effects(Value::record(r, span)).expect_err("future schema should fail");
	assert!(err.contains("unsupported schema_version"), "got: {err}");
}

#[test]
fn decode_rejects_negative_schema_version() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("schema_version", Value::int(0, span));
	r.push("effects", Value::list(vec![], span));
	let err = decode_macro_effects(Value::record(r, span)).expect_err("zero schema should fail");
	assert!(err.contains("must be >= 1"), "got: {err}");
}

#[test]
fn decode_envelope_warnings_capped() {
	let span = Span::unknown();
	let budget = DecodeBudget {
		max_effects: 2,
		..DecodeBudget::macro_defaults()
	};
	let mut r = Record::new();
	r.push("schema_version", Value::int(1, span));
	r.push("effects", Value::list(vec![], span));
	r.push(
		"warnings",
		Value::list(vec![Value::string("a", span), Value::string("b", span), Value::string("c", span)], span),
	);
	let decoded = decode_macro_effects_with_budget(Value::record(r, span), budget).expect("should decode");
	assert_eq!(decoded.warnings.len(), 2, "warnings should be capped at max_effects");
}

#[test]
fn call_limits_align_with_schema_defaults() {
	assert_eq!(DEFAULT_CALL_LIMITS.max_args, schema::DEFAULT_LIMITS.max_args);
	assert_eq!(DEFAULT_CALL_LIMITS.max_arg_len, schema::DEFAULT_LIMITS.max_string_len);
	assert_eq!(DEFAULT_CALL_LIMITS.max_env_string_len, schema::DEFAULT_LIMITS.max_string_len);
}

#[test]
fn lenient_decode_flattens_list_of_envelopes() {
	let span = Span::unknown();
	let mut stop = Record::new();
	stop.push("type", Value::string("stop", span));
	let stop_val = Value::record(stop, span);
	let env1 = envelope(span, vec![dispatch_record(span, "action", "move_right")]);
	let env2 = envelope(span, vec![stop_val.clone()]);
	let list = Value::list(vec![env1, env2], span);
	let batch = decode_effects_lenient(list, DecodeBudget::macro_defaults(), DecodeSurface::Hook).expect("should decode");
	assert_eq!(batch.effects.len(), 2);
}

#[test]
fn lenient_decode_flattens_nested_lists() {
	let span = Span::unknown();
	let mut stop = Record::new();
	stop.push("type", Value::string("stop", span));
	let stop_val = Value::record(stop, span);
	let inner = Value::list(vec![stop_val], span);
	let outer = Value::list(vec![inner, dispatch_record(span, "action", "move_right")], span);
	let batch = decode_effects_lenient(outer, DecodeBudget::macro_defaults(), DecodeSurface::Hook).expect("should decode");
	assert_eq!(batch.effects.len(), 2);
}
