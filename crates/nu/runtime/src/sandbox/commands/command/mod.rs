use xeno_invocation::schema;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, SyntaxShape, Type};

#[derive(Clone)]
pub struct CommandCommand;

impl Command for CommandCommand {
	fn name(&self) -> &str {
		"command"
	}

	fn signature(&self) -> Signature {
		Signature::build("command")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("name", SyntaxShape::String, "Command name")
			.rest("args", SyntaxShape::String, "Command arguments")
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Create a command invocation"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let name: String = call.req(engine_state, stack, 0)?;
		let args: Vec<String> = call.rest(engine_state, stack, 1)?;
		Ok(PipelineData::Value(schema::command_record(name, args, span), None))
	}
}

#[cfg(test)]
mod tests;
