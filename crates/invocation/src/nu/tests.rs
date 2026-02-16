use xeno_nu_value::{Record, Span, Value};

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
	assert!(matches!(decoded.effects[0], NuEffect::Dispatch(Invocation::Command { ref name, .. }) if name == "write"));
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
	assert!(matches!(inv, Invocation::Command { name, .. } if name == "write"));
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
