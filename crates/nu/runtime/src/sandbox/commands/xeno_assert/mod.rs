use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, SyntaxShape, Type};

#[derive(Clone)]
pub struct XenoAssertCommand;

impl Command for XenoAssertCommand {
	fn name(&self) -> &str {
		"xeno assert"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno assert")
			.input_output_types(vec![(Type::Any, Type::Any)])
			.required("predicate", SyntaxShape::Boolean, "Condition that must be true.")
			.optional(
				"message",
				SyntaxShape::String,
				"Error message if assertion fails (default: 'assertion failed').",
			)
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Assert a condition; abort evaluation if false"
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["validate", "check", "guard"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let predicate: bool = call.req(engine_state, stack, 0)?;
		let message: Option<String> = call.opt(engine_state, stack, 1)?;
		let message = message.unwrap_or_else(|| "assertion failed".to_string());

		if predicate {
			Ok(input)
		} else {
			Err(ShellError::GenericError {
				error: "xeno assert failed".into(),
				msg: message,
				span: Some(span),
				help: Some("predicate evaluated to false".into()),
				inner: vec![],
			})
		}
	}
}

#[cfg(test)]
mod tests;
