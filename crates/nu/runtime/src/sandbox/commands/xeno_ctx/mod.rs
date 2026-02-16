use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, Type};

#[derive(Clone)]
pub struct XenoCtxCommand;

impl Command for XenoCtxCommand {
	fn name(&self) -> &str {
		"xeno ctx"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno ctx")
			.input_output_types(vec![(Type::Nothing, Type::Any)])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Return the current Xeno invocation context (same as $env.XENO_CTX)"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let value = stack
			.get_env_var(engine_state, "XENO_CTX")
			.cloned()
			.unwrap_or_else(|| xeno_nu_protocol::Value::nothing(span));
		Ok(PipelineData::Value(value, None))
	}
}

#[cfg(test)]
mod tests;
