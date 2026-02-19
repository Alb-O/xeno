use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, Type};

#[derive(Clone)]
pub struct XenoSelectionGetCommand;

impl Command for XenoSelectionGetCommand {
	fn name(&self) -> &str {
		"xeno selection get"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno selection get")
			.input_output_types(vec![(Type::Nothing, Type::Record(vec![].into()))])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Return the current selection state from the invocation context"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let ctx = stack
			.get_env_var(engine_state, "XENO_CTX")
			.cloned()
			.unwrap_or_else(|| xeno_nu_protocol::Value::nothing(span));

		let selection = match &ctx {
			xeno_nu_protocol::Value::Record { val, .. } => val.get("selection").cloned().unwrap_or_else(|| xeno_nu_protocol::Value::nothing(span)),
			_ => {
				return Err(super::err(span, "xeno selection get", "no invocation context available (XENO_CTX not set)"));
			}
		};

		Ok(PipelineData::Value(selection, None))
	}
}

#[cfg(test)]
mod tests;
