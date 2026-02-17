use xeno_invocation::nu::{EFFECT_SCHEMA_VERSION, NuNotifyLevel};
use xeno_invocation::schema;
use xeno_nu_data::Record as DataRecord;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, Record, ShellError, Signature, SyntaxShape, Type, Value};

use super::{err, err_help};

#[derive(Clone)]
pub struct XenoEffectCommand;

impl Command for XenoEffectCommand {
	fn name(&self) -> &str {
		"xeno effect"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno effect")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required(
				"type",
				SyntaxShape::String,
				"Effect type: dispatch, notify, stop, edit, clipboard, state, schedule",
			)
			.rest("args", SyntaxShape::String, "Effect arguments")
			.named("count", SyntaxShape::Int, "Repeat count (dispatch action only)", None)
			.switch("extend", "Extend selection (dispatch action only)", None)
			.named("register", SyntaxShape::String, "Register, single char (dispatch action only)", None)
			.named("char", SyntaxShape::String, "Character argument, single char (dispatch action only)", None)
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Emit a validated typed effect record."
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["effect", "dispatch", "record"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let effect_type: String = call.req(engine_state, stack, 0)?;
		let args: Vec<String> = call.rest(engine_state, stack, 1)?;

		let effect = match effect_type.as_str() {
			"dispatch" => build_dispatch_effect(call, engine_state, stack, span, args)?,
			"notify" => build_notify_effect(span, args)?,
			"stop" => build_stop_effect(span, args)?,
			"edit" => build_edit_effect(span, args)?,
			"clipboard" => build_clipboard_effect(span, args)?,
			"state" => build_state_effect(span, args)?,
			"schedule" => build_schedule_effect(span, args)?,
			other => {
				return Err(err_help(
					span,
					format!("xeno effect: unknown effect type '{other}'"),
					"expected one of: dispatch, notify, stop, edit, clipboard, state, schedule",
					"valid effect types: dispatch, notify, stop, edit, clipboard, state, schedule",
				));
			}
		};

		let mut envelope = Record::new();
		envelope.push("schema_version", Value::int(EFFECT_SCHEMA_VERSION, span));
		envelope.push("effects", Value::list(vec![effect], span));
		Ok(PipelineData::Value(Value::record(envelope, span), None))
	}
}

fn build_dispatch_effect(
	call: &Call,
	engine_state: &EngineState,
	stack: &mut Stack,
	span: xeno_nu_protocol::Span,
	args: Vec<String>,
) -> Result<Value, ShellError> {
	if args.len() < 2 {
		return Err(err_help(
			span,
			"xeno effect: dispatch requires <kind> <name>",
			"missing required dispatch arguments",
			"usage: xeno effect dispatch <kind> <name> [args...]",
		));
	}

	let kind = &args[0];
	let name = &args[1];
	if name.is_empty() {
		return Err(err(span, "xeno effect: dispatch name must not be empty", "empty name"));
	}

	let mut invocation = Record::new();
	invocation.push(schema::KIND, Value::string(kind, span));
	invocation.push(schema::NAME, Value::string(name, span));

	match kind.as_str() {
		schema::KIND_ACTION => {
			let count: Option<i64> = call.get_flag(engine_state, stack, "count")?;
			let extend = call.has_flag(engine_state, stack, "extend")?;
			let register: Option<String> = call.get_flag(engine_state, stack, "register")?;
			let char_arg: Option<String> = call.get_flag(engine_state, stack, "char")?;
			if let Some(count) = count {
				invocation.push(schema::COUNT, Value::int(count, span));
			}
			if extend {
				invocation.push(schema::EXTEND, Value::bool(true, span));
			}
			if let Some(register) = register {
				invocation.push(schema::REGISTER, Value::string(register, span));
			}
			if let Some(char_arg) = char_arg {
				invocation.push(schema::CHAR, Value::string(char_arg, span));
			}
		}
		schema::KIND_COMMAND | schema::KIND_EDITOR | schema::KIND_NU => {
			invocation.push(
				schema::ARGS,
				Value::list(args.into_iter().skip(2).map(|arg| Value::string(arg, span)).collect(), span),
			);
		}
		other => {
			return Err(err_help(
				span,
				format!("xeno effect: unknown invocation kind '{other}'"),
				"expected one of: action, command, editor, nu",
				"valid kinds: action, command, editor, nu",
			));
		}
	}

	let invocation =
		DataRecord::try_from(invocation).map_err(|error| err(span, format!("xeno effect: {error}"), "invocation contains unsupported Nu value types"))?;

	let normalized = schema::validate_invocation_record(&invocation, None, &schema::DEFAULT_LIMITS, span.into()).map_err(|msg| {
		let error = format!("xeno effect: {msg}");
		err(span, error, msg)
	})?;

	let normalized_record = normalized.into_record().map_err(|error| {
		err(
			span,
			format!("xeno effect: failed to normalize dispatch effect: {error:?}"),
			"normalized dispatch shape is invalid",
		)
	})?;

	let mut effect = Record::new();
	effect.push("type", Value::string("dispatch", span));
	for (key, value) in normalized_record.iter() {
		effect.push(key, Value::from(value.clone()));
	}
	Ok(Value::record(effect, span))
}

fn build_notify_effect(span: xeno_nu_protocol::Span, args: Vec<String>) -> Result<Value, ShellError> {
	if args.len() < 2 {
		return Err(err_help(
			span,
			"xeno effect: notify requires <level> <message>",
			"missing notify arguments",
			"usage: xeno effect notify <debug|info|warn|error|success> <message>",
		));
	}

	let level = &args[0];
	let Some(parsed_level) = NuNotifyLevel::parse(level) else {
		return Err(err_help(
			span,
			format!("xeno effect: unknown notify level '{level}'"),
			"invalid notify level",
			"valid levels: debug, info, warn, error, success",
		));
	};
	let message = args.into_iter().skip(1).collect::<Vec<_>>().join(" ");
	if message.is_empty() {
		return Err(err(span, "xeno effect: notify message must not be empty", "empty message"));
	}

	let mut rec = Record::new();
	rec.push("type", Value::string("notify", span));
	rec.push("level", Value::string(parsed_level.as_str(), span));
	rec.push("message", Value::string(message, span));
	Ok(Value::record(rec, span))
}

fn build_edit_effect(span: xeno_nu_protocol::Span, args: Vec<String>) -> Result<Value, ShellError> {
	if args.len() < 2 {
		return Err(err_help(
			span,
			"xeno effect: edit requires <op> <text>",
			"missing edit arguments",
			"usage: xeno effect edit <replace_selection|replace_line> <text>",
		));
	}

	let op = args[0].clone();
	match op.as_str() {
		"replace_selection" | "replace_line" => {}
		other => {
			return Err(err_help(
				span,
				format!("xeno effect: unknown edit op '{other}'"),
				"invalid edit operation",
				"valid ops: replace_selection, replace_line",
			));
		}
	}

	let text = args.into_iter().skip(1).collect::<Vec<_>>().join(" ");

	if op == "replace_line" && text.contains(['\n', '\r']) {
		return Err(err(
			span,
			"xeno effect: replace_line text must not contain newline characters",
			"newline in replace_line text",
		));
	}

	let mut rec = Record::new();
	rec.push("type", Value::string("edit", span));
	rec.push("op", Value::string(&op, span));
	rec.push("text", Value::string(text, span));
	Ok(Value::record(rec, span))
}

fn build_schedule_effect(span: xeno_nu_protocol::Span, args: Vec<String>) -> Result<Value, ShellError> {
	if args.is_empty() {
		return Err(err_help(
			span,
			"xeno effect: schedule requires <op> ...",
			"missing schedule arguments",
			"usage: xeno effect schedule set <key> <delay_ms> <macro> [args...] | xeno effect schedule cancel <key>",
		));
	}

	let op = &args[0];
	match op.as_str() {
		"set" => {
			if args.len() < 4 {
				return Err(err_help(
					span,
					"xeno effect: schedule set requires <key> <delay_ms> <macro> [args...]",
					"missing arguments",
					"usage: xeno effect schedule set <key> <delay_ms> <macro> [args...]",
				));
			}
			let key = &args[1];
			if key.is_empty() {
				return Err(err(span, "xeno effect: schedule key must not be empty", "empty key"));
			}
			let delay_ms: u64 = args[2].parse().map_err(|_| {
				err(
					span,
					format!("xeno effect: invalid delay_ms '{}'; expected non-negative integer", args[2]),
					"invalid delay_ms",
				)
			})?;
			if delay_ms > xeno_invocation::nu::MAX_SCHEDULE_DELAY_MS {
				return Err(err(
					span,
					format!("xeno effect: delay_ms {} exceeds max {}", delay_ms, xeno_invocation::nu::MAX_SCHEDULE_DELAY_MS),
					"delay_ms too large",
				));
			}
			let macro_name = &args[3];
			if macro_name.is_empty() {
				return Err(err(span, "xeno effect: schedule macro name must not be empty", "empty macro name"));
			}
			let macro_args: Vec<Value> = args[4..].iter().map(|a| Value::string(a, span)).collect();

			let mut rec = Record::new();
			rec.push("type", Value::string("schedule", span));
			rec.push("op", Value::string("set", span));
			rec.push("key", Value::string(key, span));
			rec.push("delay_ms", Value::int(delay_ms as i64, span));
			rec.push("macro", Value::string(macro_name, span));
			rec.push("args", Value::list(macro_args, span));
			Ok(Value::record(rec, span))
		}
		"cancel" => {
			if args.len() < 2 {
				return Err(err_help(
					span,
					"xeno effect: schedule cancel requires <key>",
					"missing key",
					"usage: xeno effect schedule cancel <key>",
				));
			}
			let key = &args[1];
			if key.is_empty() {
				return Err(err(span, "xeno effect: schedule key must not be empty", "empty key"));
			}

			let mut rec = Record::new();
			rec.push("type", Value::string("schedule", span));
			rec.push("op", Value::string("cancel", span));
			rec.push("key", Value::string(key, span));
			Ok(Value::record(rec, span))
		}
		other => Err(err_help(
			span,
			format!("xeno effect: unknown schedule op '{other}'"),
			"invalid schedule operation",
			"valid ops: set, cancel",
		)),
	}
}

fn build_state_effect(span: xeno_nu_protocol::Span, args: Vec<String>) -> Result<Value, ShellError> {
	if args.is_empty() {
		return Err(err_help(
			span,
			"xeno effect: state requires <op> <key> [value...]",
			"missing state arguments",
			"usage: xeno effect state set <key> <value...> | xeno effect state unset <key>",
		));
	}

	let op = &args[0];
	match op.as_str() {
		"set" => {
			if args.len() < 3 {
				return Err(err_help(
					span,
					"xeno effect: state set requires <key> <value...>",
					"missing key or value",
					"usage: xeno effect state set <key> <value...>",
				));
			}
			let key = &args[1];
			if key.is_empty() {
				return Err(err(span, "xeno effect: state key must not be empty", "empty key"));
			}
			let value = args[2..].join(" ");

			let mut rec = Record::new();
			rec.push("type", Value::string("state", span));
			rec.push("op", Value::string("set", span));
			rec.push("key", Value::string(key, span));
			rec.push("value", Value::string(value, span));
			Ok(Value::record(rec, span))
		}
		"unset" => {
			if args.len() < 2 {
				return Err(err_help(
					span,
					"xeno effect: state unset requires <key>",
					"missing key",
					"usage: xeno effect state unset <key>",
				));
			}
			let key = &args[1];
			if key.is_empty() {
				return Err(err(span, "xeno effect: state key must not be empty", "empty key"));
			}

			let mut rec = Record::new();
			rec.push("type", Value::string("state", span));
			rec.push("op", Value::string("unset", span));
			rec.push("key", Value::string(key, span));
			Ok(Value::record(rec, span))
		}
		other => Err(err_help(
			span,
			format!("xeno effect: unknown state op '{other}'"),
			"invalid state operation",
			"valid ops: set, unset",
		)),
	}
}

fn build_clipboard_effect(span: xeno_nu_protocol::Span, args: Vec<String>) -> Result<Value, ShellError> {
	let text = args.join(" ");

	let mut rec = Record::new();
	rec.push("type", Value::string("clipboard", span));
	rec.push("text", Value::string(text, span));
	Ok(Value::record(rec, span))
}

fn build_stop_effect(span: xeno_nu_protocol::Span, args: Vec<String>) -> Result<Value, ShellError> {
	if !args.is_empty() {
		return Err(err_help(
			span,
			"xeno effect: stop does not take arguments",
			"unexpected arguments",
			"usage: xeno effect stop",
		));
	}

	let mut rec = Record::new();
	rec.push("type", Value::string("stop", span));
	Ok(Value::record(rec, span))
}

#[cfg(test)]
mod tests;
