use xeno_invocation::nu::{DecodeBudget, NuEffect};
use xeno_invocation::schema;
use xeno_nu_data::Value as DataValue;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, Record, ShellError, Signature, Type, Value};

use super::err;

#[derive(Clone)]
pub struct XenoEmitManyCommand;

impl Command for XenoEmitManyCommand {
	fn name(&self) -> &str {
		"xeno effects normalize"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno effects normalize")
			.input_output_types(vec![
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any))),
				(Type::Any, Type::List(Box::new(Type::Any))),
			])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Validate and normalize typed effect records. Accepts record, list, or batch envelope."
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let value = input
			.into_value(span)
			.map_err(|e| err(span, format!("xeno effects normalize: {e}"), "failed to collect input"))?;
		let value =
			DataValue::try_from(value).map_err(|e| err(span, format!("xeno effects normalize: {e}"), "unsupported Nu value type for effect decoding"))?;

		let batch = xeno_invocation::nu::decode_hook_effects_with_budget(value, DecodeBudget::macro_defaults())
			.map_err(|msg| err(span, format!("xeno effects normalize: {msg}"), msg))?;

		let out = batch.effects.into_iter().map(|effect| encode_effect(effect, span)).collect();
		Ok(PipelineData::Value(Value::list(out, span), None))
	}
}

fn encode_effect(effect: NuEffect, span: xeno_nu_protocol::Span) -> Value {
	match effect {
		NuEffect::Dispatch(invocation) => {
			let mut rec = Record::new();
			rec.push("type", Value::string("dispatch", span));
			match invocation {
				xeno_invocation::Invocation::Action { name, count, extend, register } => {
					rec.push(schema::KIND, Value::string(schema::KIND_ACTION, span));
					rec.push(schema::NAME, Value::string(name, span));
					rec.push(schema::COUNT, Value::int(count as i64, span));
					rec.push(schema::EXTEND, Value::bool(extend, span));
					if let Some(register) = register {
						rec.push(schema::REGISTER, Value::string(register.to_string(), span));
					}
				}
				xeno_invocation::Invocation::ActionWithChar {
					name,
					count,
					extend,
					register,
					char_arg,
				} => {
					rec.push(schema::KIND, Value::string(schema::KIND_ACTION, span));
					rec.push(schema::NAME, Value::string(name, span));
					rec.push(schema::COUNT, Value::int(count as i64, span));
					rec.push(schema::EXTEND, Value::bool(extend, span));
					if let Some(register) = register {
						rec.push(schema::REGISTER, Value::string(register.to_string(), span));
					}
					rec.push(schema::CHAR, Value::string(char_arg.to_string(), span));
				}
				xeno_invocation::Invocation::Command { name, args } => {
					rec.push(schema::KIND, Value::string(schema::KIND_COMMAND, span));
					rec.push(schema::NAME, Value::string(name, span));
					rec.push(schema::ARGS, Value::list(args.into_iter().map(|arg| Value::string(arg, span)).collect(), span));
				}
				xeno_invocation::Invocation::EditorCommand { name, args } => {
					rec.push(schema::KIND, Value::string(schema::KIND_EDITOR, span));
					rec.push(schema::NAME, Value::string(name, span));
					rec.push(schema::ARGS, Value::list(args.into_iter().map(|arg| Value::string(arg, span)).collect(), span));
				}
				xeno_invocation::Invocation::Nu { name, args } => {
					rec.push(schema::KIND, Value::string(schema::KIND_NU, span));
					rec.push(schema::NAME, Value::string(name, span));
					rec.push(schema::ARGS, Value::list(args.into_iter().map(|arg| Value::string(arg, span)).collect(), span));
				}
			}
			Value::record(rec, span)
		}
		NuEffect::Notify { level, message } => {
			let mut rec = Record::new();
			rec.push("type", Value::string("notify", span));
			rec.push("level", Value::string(level.as_str(), span));
			rec.push("message", Value::string(message, span));
			Value::record(rec, span)
		}
		NuEffect::StopPropagation => {
			let mut rec = Record::new();
			rec.push("type", Value::string("stop", span));
			Value::record(rec, span)
		}
	}
}

#[cfg(test)]
mod tests;
