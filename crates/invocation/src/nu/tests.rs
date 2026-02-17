use xeno_nu_data::{Record, Span, Value};

use super::*;

fn dispatch_record(span: Span, kind: &str, name: &str) -> Value {
	let mut r = Record::new();
	r.push("type", Value::string("dispatch", span));
	r.push("kind", Value::string(kind, span));
	r.push("name", Value::string(name, span));
	Value::record(r, span)
}

#[test]
fn macro_decode_allows_nothing_return() {
	let decoded = decode_macro_effects(Value::nothing(Span::unknown())).expect("nothing should decode");
	assert!(decoded.effects.is_empty());
}

#[test]
fn macro_decode_accepts_dispatch_record() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(dispatch_record(span, "action", "move_right")).expect("dispatch should decode");
	assert_eq!(decoded.schema_version, 1);
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(decoded.effects[0], NuEffect::Dispatch(Invocation::Action { ref name, .. }) if name == "move_right"));
}

#[test]
fn macro_decode_rejects_stop_effect() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("stop", span));
	let err = decode_macro_effects(Value::record(r, span)).expect_err("macro stop should fail");
	assert!(err.contains("only allowed in hook"), "got: {err}");
}

#[test]
fn hook_decode_accepts_stop_effect() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("stop", span));
	let decoded = decode_hook_effects(Value::record(r, span)).expect("hook stop should decode");
	assert!(decoded.has_stop_propagation());
}

#[test]
fn decode_envelope_list() {
	let span = Span::unknown();
	let mut envelope = Record::new();
	envelope.push("schema_version", Value::int(3, span));
	envelope.push("effects", Value::list(vec![dispatch_record(span, "command", "write")], span));
	let decoded = decode_macro_effects(Value::record(envelope, span)).expect("envelope should decode");
	assert_eq!(decoded.schema_version, 3);
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
fn capability_for_dispatch_matches_invocation_kind() {
	assert_eq!(
		required_capability_for_effect(&NuEffect::Dispatch(Invocation::action("move_right"))),
		NuCapability::DispatchAction
	);
	assert_eq!(
		required_capability_for_effect(&NuEffect::Dispatch(Invocation::command("write", vec![]))),
		NuCapability::DispatchCommand
	);
	assert_eq!(
		required_capability_for_effect(&NuEffect::Notify {
			level: NuNotifyLevel::Warn,
			message: "warn".to_string()
		}),
		NuCapability::Notify
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
	let decoded = decode_macro_effects(edit_record(span, "replace_selection", "HELLO")).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::EditText { op: NuTextEditOp::ReplaceSelection, text } if text == "HELLO"
	));
}

#[test]
fn decode_edit_replace_line() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(edit_record(span, "replace_line", "new content")).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::EditText { op: NuTextEditOp::ReplaceLine, text } if text == "new content"
	));
}

#[test]
fn decode_edit_replace_line_rejects_newline() {
	let span = Span::unknown();
	let err = decode_macro_effects(edit_record(span, "replace_line", "line1\nline2")).expect_err("newline should fail");
	assert!(err.contains("newline"), "got: {err}");
}

#[test]
fn decode_edit_replace_selection_allows_empty_text() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(edit_record(span, "replace_selection", "")).expect("empty text should decode");
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::EditText { op: NuTextEditOp::ReplaceSelection, text } if text.is_empty()
	));
}

#[test]
fn decode_edit_unknown_op_errors() {
	let span = Span::unknown();
	let err = decode_macro_effects(edit_record(span, "unknown_op", "text")).expect_err("unknown op should fail");
	assert!(err.contains("unknown edit op"), "got: {err}");
}

#[test]
fn capability_for_edit_text() {
	assert_eq!(
		required_capability_for_effect(&NuEffect::EditText {
			op: NuTextEditOp::ReplaceSelection,
			text: "x".into()
		}),
		NuCapability::EditText
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
	let decoded = decode_macro_effects(clipboard_record(span, "copied text")).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::SetClipboard { text } if text == "copied text"
	));
}

#[test]
fn decode_clipboard_empty_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(clipboard_record(span, "")).expect("empty clipboard should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::SetClipboard { text } if text.is_empty()
	));
}

#[test]
fn capability_for_set_clipboard() {
	assert_eq!(
		required_capability_for_effect(&NuEffect::SetClipboard { text: "x".into() }),
		NuCapability::SetClipboard
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
	let decoded = decode_macro_effects(state_set_record(span, "foo", "bar")).expect("should decode");
	assert_eq!(decoded.effects.len(), 1);
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::StateSet { key, value } if key == "foo" && value == "bar"
	));
}

#[test]
fn decode_state_set_empty_value_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(state_set_record(span, "foo", "")).expect("empty value should decode");
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::StateSet { key, value } if key == "foo" && value.is_empty()
	));
}

#[test]
fn decode_state_unset_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(state_unset_record(span, "foo")).expect("should decode");
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
	let err = decode_macro_effects(Value::record(r, span)).expect_err("bad op should fail");
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
	let err = decode_macro_effects(Value::record(r, span)).expect_err("empty key should fail");
	assert!(err.contains("must not be empty"), "got: {err}");
}

#[test]
fn capability_for_write_state() {
	assert_eq!(
		required_capability_for_effect(&NuEffect::StateSet {
			key: "k".into(),
			value: "v".into()
		}),
		NuCapability::WriteState
	);
	assert_eq!(
		required_capability_for_effect(&NuEffect::StateUnset { key: "k".into() }),
		NuCapability::WriteState
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
	let decoded = decode_macro_effects(schedule_set_record(span, "autosave", 750, "save-all", vec![])).expect("should decode");
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
	let decoded = decode_macro_effects(schedule_set_record(span, "fmt", 300, "format-buffer", vec!["--quiet"])).expect("should decode");
	assert!(matches!(
		&decoded.effects[0],
		NuEffect::ScheduleSet { key, name, args, .. }
			if key == "fmt" && name == "format-buffer" && args == &["--quiet"]
	));
}

#[test]
fn decode_schedule_cancel_ok() {
	let span = Span::unknown();
	let decoded = decode_macro_effects(schedule_cancel_record(span, "autosave")).expect("should decode");
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
	let err = decode_macro_effects(Value::record(r, span)).expect_err("excessive delay should fail");
	assert!(err.contains("exceeds max"), "got: {err}");
}

#[test]
fn decode_schedule_bad_op_errors() {
	let span = Span::unknown();
	let mut r = Record::new();
	r.push("type", Value::string("schedule", span));
	r.push("op", Value::string("pause", span));
	r.push("key", Value::string("k", span));
	let err = decode_macro_effects(Value::record(r, span)).expect_err("bad op should fail");
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
	let err = decode_macro_effects(Value::record(r, span)).expect_err("empty key should fail");
	assert!(err.contains("must not be empty"), "got: {err}");
}

#[test]
fn capability_for_schedule_macro() {
	assert_eq!(
		required_capability_for_effect(&NuEffect::ScheduleSet {
			key: "k".into(),
			delay_ms: 100,
			name: "m".into(),
			args: vec![]
		}),
		NuCapability::ScheduleMacro
	);
	assert_eq!(
		required_capability_for_effect(&NuEffect::ScheduleCancel { key: "k".into() }),
		NuCapability::ScheduleMacro
	);
}
