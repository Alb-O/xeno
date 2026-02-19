use xeno_invocation::nu::{DecodeBudget, EFFECT_SCHEMA_VERSION, NuEffect};
use xeno_invocation::schema;
use xeno_nu_data::Value as DataValue;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, Record, ShellError, Signature, Type, Value};

use super::err;

#[derive(Clone)]
pub struct XenoEffectsNormalizeCommand;

impl Command for XenoEffectsNormalizeCommand {
	fn name(&self) -> &str {
		"xeno effects normalize"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno effects normalize")
			.input_output_types(vec![
				(Type::List(Box::new(Type::Any)), Type::Record(Box::new([]))),
				(Type::Record(Box::new([])), Type::Record(Box::new([]))),
				(Type::Nothing, Type::Record(Box::new([]))),
				(Type::Any, Type::Record(Box::new([]))),
			])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Validate and normalize typed effect records into an envelope. Accepts record, list, nothing, or envelope."
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let value = input
			.into_value(span)
			.map_err(|e| err(span, format!("xeno effects normalize: {e}"), "failed to collect input"))?;
		let value =
			DataValue::try_from(value).map_err(|e| err(span, format!("xeno effects normalize: {e}"), "unsupported Nu value type for effect decoding"))?;

		// Accept any shape (bare record, list, nothing, or envelope) via the lenient decoder
		// which accepts bare effect records and lists in addition to envelopes.
		let batch = xeno_invocation::nu::decode_effects_lenient(value, DecodeBudget::macro_defaults(), xeno_invocation::nu::DecodeSurface::Hook)
			.map_err(|msg| err(span, format!("xeno effects normalize: {msg}"), msg))?;

		let effects: Vec<Value> = batch.effects.into_iter().map(|effect| encode_effect(effect, span)).collect();
		let mut envelope = Record::new();
		envelope.push("schema_version", Value::int(EFFECT_SCHEMA_VERSION, span));
		envelope.push("effects", Value::list(effects, span));
		if !batch.warnings.is_empty() {
			envelope.push(
				"warnings",
				Value::list(batch.warnings.into_iter().map(|w| Value::string(w, span)).collect(), span),
			);
		}
		Ok(PipelineData::Value(Value::record(envelope, span), None))
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
				xeno_invocation::Invocation::Command(xeno_invocation::CommandInvocation { name, args, route }) => {
					let kind = if route == xeno_invocation::CommandRoute::Editor {
						schema::KIND_EDITOR
					} else {
						schema::KIND_COMMAND
					};
					rec.push(schema::KIND, Value::string(kind, span));
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
		NuEffect::SetClipboard { text } => {
			let mut rec = Record::new();
			rec.push("type", Value::string("clipboard", span));
			rec.push("text", Value::string(text, span));
			Value::record(rec, span)
		}
		NuEffect::StateSet { key, value } => {
			let mut rec = Record::new();
			rec.push("type", Value::string("state", span));
			rec.push("op", Value::string("set", span));
			rec.push("key", Value::string(key, span));
			rec.push("value", Value::string(value, span));
			Value::record(rec, span)
		}
		NuEffect::StateUnset { key } => {
			let mut rec = Record::new();
			rec.push("type", Value::string("state", span));
			rec.push("op", Value::string("unset", span));
			rec.push("key", Value::string(key, span));
			Value::record(rec, span)
		}
		NuEffect::ScheduleSet { key, delay_ms, name, args } => {
			let mut rec = Record::new();
			rec.push("type", Value::string("schedule", span));
			rec.push("op", Value::string("set", span));
			rec.push("key", Value::string(key, span));
			rec.push("delay_ms", Value::int(delay_ms as i64, span));
			rec.push("macro", Value::string(name, span));
			rec.push("args", Value::list(args.into_iter().map(|a| Value::string(a, span)).collect(), span));
			Value::record(rec, span)
		}
		NuEffect::ScheduleCancel { key } => {
			let mut rec = Record::new();
			rec.push("type", Value::string("schedule", span));
			rec.push("op", Value::string("cancel", span));
			rec.push("key", Value::string(key, span));
			Value::record(rec, span)
		}
		NuEffect::EditText { op, text } => {
			let mut rec = Record::new();
			rec.push("type", Value::string("edit", span));
			rec.push(
				"op",
				Value::string(
					match op {
						xeno_invocation::nu::NuTextEditOp::ReplaceSelection => "replace_selection",
						xeno_invocation::nu::NuTextEditOp::ReplaceLine => "replace_line",
					},
					span,
				),
			);
			rec.push("text", Value::string(text, span));
			Value::record(rec, span)
		}
	}
}

#[cfg(test)]
mod tests;
