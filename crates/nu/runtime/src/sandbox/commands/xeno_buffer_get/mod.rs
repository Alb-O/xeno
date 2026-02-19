use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, Record, ShellError, Signature, SyntaxShape, Type, Value};

use crate::host::with_host;

#[derive(Clone)]
pub struct XenoBufferGetCommand;

impl Command for XenoBufferGetCommand {
	fn name(&self) -> &str {
		"xeno buffer get"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno buffer get")
			.input_output_types(vec![(Type::Nothing, Type::Record(vec![].into()))])
			.named("id", SyntaxShape::Int, "buffer ID (default: active buffer)", None)
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Return metadata about a buffer from the host"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let id = call.get_flag::<i64>(engine_state, stack, "id")?;

		let meta = with_host(|host| host.buffer_get(id))
			.ok_or_else(|| super::err(span, "xeno buffer get", "no host available (command can only be used during Nu evaluation)"))?
			.map_err(|e| super::err(span, "xeno buffer get", e.0))?;

		let mut record = Record::new();
		record.push("path", meta.path.map_or_else(|| Value::nothing(span), |p| Value::string(p, span)));
		record.push("file_type", meta.file_type.map_or_else(|| Value::nothing(span), |ft| Value::string(ft, span)));
		record.push("readonly", Value::bool(meta.readonly, span));
		record.push("modified", Value::bool(meta.modified, span));
		record.push("line_count", Value::int(meta.line_count as i64, span));

		Ok(PipelineData::Value(Value::record(record, span), None))
	}
}

#[cfg(test)]
mod tests;
