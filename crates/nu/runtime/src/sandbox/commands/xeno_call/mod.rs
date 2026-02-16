use xeno_invocation::schema;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, Record, ShellError, Signature, SyntaxShape, Type, Value};

use super::err;

#[derive(Clone)]
pub struct XenoCallCommand;

impl Command for XenoCallCommand {
	fn name(&self) -> &str {
		"xeno call"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno call")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("name", SyntaxShape::String, "Nu function name")
			.rest("args", SyntaxShape::String, "Function arguments")
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Create a typed dispatch effect targeting a Nu macro function."
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let name: String = call.req(engine_state, stack, 0)?;
		if name.is_empty() {
			return Err(err(span, "xeno call: name must not be empty", "empty name"));
		}
		let args: Vec<String> = call.rest(engine_state, stack, 1)?;
		let mut rec = Record::new();
		rec.push("type", Value::string("dispatch", span));
		rec.push(schema::KIND, Value::string(schema::KIND_NU, span));
		rec.push(schema::NAME, Value::string(name, span));
		rec.push(schema::ARGS, Value::list(args.into_iter().map(|arg| Value::string(arg, span)).collect(), span));
		Ok(PipelineData::Value(Value::record(rec, span), None))
	}
}

#[cfg(test)]
mod tests;
