use xeno_invocation::schema;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, SyntaxShape, Type};

#[derive(Clone)]
pub struct NuRunCommand;

impl Command for NuRunCommand {
	fn name(&self) -> &str {
		"nu run"
	}

	fn signature(&self) -> Signature {
		Signature::build("nu run")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("name", SyntaxShape::String, "Nu function name")
			.rest("args", SyntaxShape::String, "Function arguments")
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Create a Nu macro invocation"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let name: String = call.req(engine_state, stack, 0)?;
		let args: Vec<String> = call.rest(engine_state, stack, 1)?;
		Ok(PipelineData::Value(schema::nu_record(name, args, span), None))
	}
}

#[cfg(test)]
mod tests;
