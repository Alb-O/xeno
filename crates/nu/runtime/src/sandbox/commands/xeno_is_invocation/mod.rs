use xeno_invocation::nu::DecodeBudget;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, Type, Value};

use super::err;

#[derive(Clone)]
pub struct XenoIsInvocationCommand;

impl Command for XenoIsInvocationCommand {
	fn name(&self) -> &str {
		"xeno is-effect"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno is-effect")
			.input_output_types(vec![(Type::Any, Type::Bool)])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Check if the pipeline value is a valid typed effect record."
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let value = input
			.into_value(span)
			.map_err(|e| err(span, format!("xeno is-effect: {e}"), "failed to collect input"))?;

		let is_effect = xeno_invocation::nu::decode_hook_effects_with_budget(
			value,
			DecodeBudget {
				max_effects: 1,
				..DecodeBudget::macro_defaults()
			},
		)
		.map(|batch| batch.effects.len() == 1)
		.unwrap_or(false);

		Ok(PipelineData::Value(Value::bool(is_effect, span), None))
	}
}

#[cfg(test)]
mod tests;
