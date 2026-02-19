use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, Record, ShellError, Signature, SyntaxShape, Type, Value};

use crate::host::{LineColRange, with_host};

/// Default max bytes for buffer text output.
const DEFAULT_MAX_BYTES: usize = 64 * 1024;

/// Hard ceiling — scripts cannot request more than this regardless of `--max-bytes`.
const HARD_MAX_BYTES: usize = 256 * 1024;

#[derive(Clone)]
pub struct XenoBufferTextCommand;

impl Command for XenoBufferTextCommand {
	fn name(&self) -> &str {
		"xeno buffer text"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno buffer text")
			.input_output_types(vec![(Type::Nothing, Type::Record(vec![].into()))])
			.named("id", SyntaxShape::Int, "buffer ID (default: active buffer)", None)
			.named("start-line", SyntaxShape::Int, "start line (0-indexed)", None)
			.named("start-col", SyntaxShape::Int, "start column (0-indexed)", None)
			.named("end-line", SyntaxShape::Int, "end line (0-indexed)", None)
			.named("end-col", SyntaxShape::Int, "end column (0-indexed)", None)
			.named("max-bytes", SyntaxShape::Int, "maximum bytes to return (default 64KB)", None)
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Return bounded text from the active buffer"
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, _input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;

		let id = call.get_flag::<i64>(engine_state, stack, "id")?;
		let max_bytes = call
			.get_flag::<i64>(engine_state, stack, "max-bytes")?
			.map(|v| v.max(0) as usize)
			.unwrap_or(DEFAULT_MAX_BYTES)
			.min(HARD_MAX_BYTES);

		let range = parse_range(engine_state, stack, call)?;

		let chunk = with_host(|host| host.buffer_text(id, range, max_bytes))
			.ok_or_else(|| super::err(span, "xeno buffer text", "no host available (command can only be used during Nu evaluation)"))?
			.map_err(|e| super::err(span, "xeno buffer text", e.0))?;

		let mut record = Record::new();
		record.push("text", Value::string(chunk.text, span));
		record.push("truncated", Value::bool(chunk.truncated, span));

		Ok(PipelineData::Value(Value::record(record, span), None))
	}
}

fn parse_range(engine_state: &EngineState, stack: &mut Stack, call: &Call) -> Result<Option<LineColRange>, ShellError> {
	let start_line = call.get_flag::<i64>(engine_state, stack, "start-line")?;
	let start_col = call.get_flag::<i64>(engine_state, stack, "start-col")?;
	let end_line = call.get_flag::<i64>(engine_state, stack, "end-line")?;
	let end_col = call.get_flag::<i64>(engine_state, stack, "end-col")?;

	match (start_line, end_line) {
		(Some(sl), Some(el)) => Ok(Some(LineColRange {
			start_line: sl.max(0) as usize,
			start_col: start_col.unwrap_or(0i64).max(0) as usize,
			end_line: el.max(0) as usize,
			end_col: end_col.unwrap_or(i64::MAX).max(0) as usize,
		})),
		(None, None) if start_col.is_none() && end_col.is_none() => Ok(None),
		_ => Err(super::err(
			call.head,
			"xeno buffer text",
			"--start-line and --end-line must both be provided for a range query",
		)),
	}
}

#[cfg(test)]
mod tests;
