use xeno_invocation::schema;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, Type, Value};

#[derive(Clone)]
pub struct XenoIsInvocationCommand;

impl Command for XenoIsInvocationCommand {
	fn name(&self) -> &str {
		"xeno is-invocation"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno is-invocation")
			.input_output_types(vec![(Type::Any, Type::Bool)])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Check if the pipeline value is a valid invocation record (has kind + name)."
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let value = input.into_value(span).map_err(|e| ShellError::GenericError {
			error: format!("xeno is-invocation: {e}"),
			msg: "failed to collect input".into(),
			span: Some(span),
			help: None,
			inner: vec![],
		})?;

		let is_invocation = match &value {
			Value::Record { val, .. } => {
				let kind_ok = val
					.get(schema::KIND)
					.and_then(|v| v.as_str().ok())
					.is_some_and(|k| matches!(k, "action" | "command" | "editor" | "nu"));
				let name_ok = val.get(schema::NAME).and_then(|v| v.as_str().ok()).is_some();
				kind_ok && name_ok
			}
			_ => false,
		};

		Ok(PipelineData::Value(Value::bool(is_invocation, span), None))
	}
}

#[cfg(test)]
mod tests;
