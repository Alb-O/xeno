use xeno_invocation::schema;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, Type, Value};

#[derive(Clone)]
pub struct XenoEmitManyCommand;

impl Command for XenoEmitManyCommand {
	fn name(&self) -> &str {
		"xeno emit-many"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno emit-many")
			.input_output_types(vec![
				(Type::List(Box::new(Type::Any)), Type::List(Box::new(Type::Any))),
				(Type::Any, Type::List(Box::new(Type::Any))),
			])
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Validate and normalize a list of invocation records. Accepts a single record or a list."
	}

	fn run(&self, _engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let value = input.into_value(span).map_err(|e| ShellError::GenericError {
			error: format!("xeno emit-many: {e}"),
			msg: "failed to collect input".into(),
			span: Some(span),
			help: None,
			inner: vec![],
		})?;

		let items = match value {
			Value::Record { .. } => vec![value],
			Value::List { vals, .. } => vals,
			Value::Nothing { .. } => return Ok(PipelineData::Value(Value::list(vec![], span), None)),
			other => {
				return Err(ShellError::GenericError {
					error: "xeno emit-many: expected record or list of records".into(),
					msg: format!("got {}", other.get_type()),
					span: Some(span),
					help: None,
					inner: vec![],
				});
			}
		};

		let limits = &schema::DEFAULT_LIMITS;
		if items.len() > limits.max_invocations {
			return Err(ShellError::GenericError {
				error: format!("xeno emit-many: {} items exceeds limit of {}", items.len(), limits.max_invocations),
				msg: "too many invocations".into(),
				span: Some(span),
				help: None,
				inner: vec![],
			});
		}

		let mut out = Vec::with_capacity(items.len());
		for (idx, item) in items.into_iter().enumerate() {
			let rec = item.into_record().map_err(|_| ShellError::GenericError {
				error: format!("xeno emit-many: items[{idx}] must be a record"),
				msg: "expected record".into(),
				span: Some(span),
				help: None,
				inner: vec![],
			})?;
			let normalized = schema::validate_invocation_record(&rec, Some(idx), limits, span).map_err(|msg| ShellError::GenericError {
				error: format!("xeno emit-many: {msg}"),
				msg: msg.clone(),
				span: Some(span),
				help: None,
				inner: vec![],
			})?;
			out.push(normalized);
		}

		Ok(PipelineData::Value(Value::list(out, span), None))
	}
}

#[cfg(test)]
mod tests;
