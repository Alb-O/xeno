use xeno_invocation::schema;
use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, SyntaxShape, Type};

#[derive(Clone)]
pub struct ActionCommand;

impl Command for ActionCommand {
	fn name(&self) -> &str {
		"action"
	}

	fn signature(&self) -> Signature {
		Signature::build("action")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.required("name", SyntaxShape::String, "Action name")
			.named("count", SyntaxShape::Int, "Repeat count", None)
			.switch("extend", "Extend selection", None)
			.named("register", SyntaxShape::String, "Register (single char)", None)
			.named("char", SyntaxShape::String, "Character argument (single char)", None)
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Create an action invocation"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let name: String = call.req(engine_state, stack, 0)?;
		let count: Option<i64> = call.get_flag(engine_state, stack, "count")?;
		let extend = call.has_flag(engine_state, stack, "extend")?;
		let register: Option<String> = call.get_flag(engine_state, stack, "register")?;
		let char_arg: Option<String> = call.get_flag(engine_state, stack, "char")?;

		let count = count.map(|c| c.max(1)).unwrap_or(1);
		let register = super::parse_single_char(register, "register", span)?;
		let char_arg = super::parse_single_char(char_arg, "char", span)?;

		Ok(PipelineData::Value(schema::action_record(name, count, extend, register, char_arg, span), None))
	}
}

#[cfg(test)]
mod tests;
