use xeno_invocation::schema;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, SyntaxShape, Type};

#[derive(Clone)]
pub struct EditorCommand;

impl Command for EditorCommand {
	fn name(&self) -> &str {
		"editor"
	}

	fn signature(&self) -> Signature {
		Signature::build("editor")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("name", SyntaxShape::String, "Editor command name")
			.rest("args", SyntaxShape::String, "Command arguments")
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Create an editor command invocation"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let name: String = call.req(engine_state, stack, 0)?;
		let args: Vec<String> = call.rest(engine_state, stack, 1)?;
		Ok(PipelineData::Value(schema::editor_record(name, args, span), None))
	}
}

#[cfg(test)]
mod tests;
