use xeno_invocation::nu::NuNotifyLevel;
use xeno_invocation::schema;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, Record, ShellError, Signature, SyntaxShape, Type, Value};

use super::{err, err_help};

#[derive(Clone)]
pub struct XenoEmitCommand;

impl Command for XenoEmitCommand {
	fn name(&self) -> &str {
		"xeno effect"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno effect")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("type", SyntaxShape::String, "Effect type: dispatch, notify, stop")
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
			other => {
				return Err(err_help(
					span,
					format!("xeno effect: unknown effect type '{other}'"),
					"expected one of: dispatch, notify, stop",
					"valid effect types: dispatch, notify, stop",
				));
			}
		};

		Ok(PipelineData::Value(effect, None))
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

	let normalized = schema::validate_invocation_record(&invocation, None, &schema::DEFAULT_LIMITS, span).map_err(|msg| {
		let error = format!("xeno effect: {msg}");
		err(span, error, msg)
	})?;

	let normalized_record = normalized.into_record().map_err(|error| {
		err(
			span,
			format!("xeno effect: failed to normalize dispatch effect: {error}"),
			"normalized dispatch shape is invalid",
		)
	})?;

	let mut effect = Record::new();
	effect.push("type", Value::string("dispatch", span));
	for (key, value) in normalized_record.iter() {
		effect.push(key, value.clone());
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
