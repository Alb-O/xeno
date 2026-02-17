use xeno_nu_engine::CallExt;
use xeno_nu_protocol::engine::{Call, Command, EngineState, Stack};
use xeno_nu_protocol::{Category, PipelineData, ShellError, Signature, SyntaxShape, Type, Value};

use super::err_help;

#[derive(Clone)]
pub struct XenoLogCommand;

impl Command for XenoLogCommand {
	fn name(&self) -> &str {
		"xeno log"
	}

	fn signature(&self) -> Signature {
		Signature::build("xeno log")
			.input_output_types(vec![(Type::Any, Type::Any)])
			.required("label", SyntaxShape::String, "Log label for identifying the message")
			.named("level", SyntaxShape::String, "Log level: debug|info|warn|error (default: debug)", Some('l'))
			.category(Category::Custom("xeno".into()))
	}

	fn description(&self) -> &str {
		"Log the pipeline value and pass it through unchanged"
	}

	fn search_terms(&self) -> Vec<&str> {
		vec!["debug", "print", "trace"]
	}

	fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
		let span = call.head;
		let label: String = call.req(engine_state, stack, 0)?;
		let level: Option<String> = call.get_flag(engine_state, stack, "level")?;
		let level_str = level.as_deref().unwrap_or("debug");

		match level_str {
			"debug" | "info" | "warn" | "error" => {}
			other => {
				return Err(err_help(
					span,
					format!("invalid log level: '{other}'"),
					"expected one of: debug, info, warn, error",
					"valid levels: debug|info|warn|error",
				));
			}
		}

		let summary = match &input {
			PipelineData::Value(v, ..) => summarize_value(v),
			PipelineData::ListStream(..) => "<list stream>".to_string(),
			PipelineData::ByteStream(s, ..) => format!("<byte stream: {}>", s.type_().describe()),
			PipelineData::Empty => "<empty>".to_string(),
		};

		match level_str {
			"info" => tracing::info!(label = %label, "{summary}"),
			"warn" => tracing::warn!(label = %label, "{summary}"),
			"error" => tracing::error!(label = %label, "{summary}"),
			_ => tracing::debug!(label = %label, "{summary}"),
		}

		Ok(input)
	}
}

const MAX_LOG_STRING: usize = 200;
const MAX_LOG_LIST: usize = 50;
const MAX_LOG_RECORD: usize = 50;
const MAX_LOG_NODES: usize = 200;
const MAX_LOG_OUT_BYTES: usize = xeno_invocation::nu::DEFAULT_CALL_LIMITS.max_env_string_len;

fn trunc_utf8(s: &str, max_bytes: usize) -> (&str, bool) {
	if s.len() <= max_bytes {
		return (s, false);
	}
	let end = (0..=max_bytes).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0);
	(&s[..end], true)
}

fn summarize_value(v: &Value) -> String {
	let mut buf = String::new();
	let mut nodes = 0usize;
	summarize_inner(v, &mut buf, &mut nodes);
	if buf.len() > MAX_LOG_OUT_BYTES {
		let (trunc, _) = trunc_utf8(&buf, MAX_LOG_OUT_BYTES);
		let mut out = trunc.to_string();
		out.push_str("...");
		return out;
	}
	buf
}

fn summarize_inner(v: &Value, buf: &mut String, nodes: &mut usize) {
	use std::fmt::Write;
	*nodes += 1;
	if *nodes > MAX_LOG_NODES || buf.len() > MAX_LOG_OUT_BYTES {
		buf.push_str("...");
		return;
	}
	match v {
		Value::Nothing { .. } => buf.push_str("null"),
		Value::Bool { val, .. } => {
			let _ = write!(buf, "{val}");
		}
		Value::Int { val, .. } => {
			let _ = write!(buf, "{val}");
		}
		Value::Float { val, .. } => {
			let _ = write!(buf, "{val}");
		}
		Value::String { val, .. } => {
			buf.push('"');
			let (s, truncated) = trunc_utf8(val, MAX_LOG_STRING);
			buf.push_str(s);
			if truncated {
				buf.push_str("...");
			}
			buf.push('"');
		}
		Value::List { vals, .. } => {
			buf.push('[');
			let limit = vals.len().min(MAX_LOG_LIST);
			for (i, item) in vals.iter().take(limit).enumerate() {
				if i > 0 {
					buf.push_str(", ");
				}
				summarize_inner(item, buf, nodes);
				if *nodes > MAX_LOG_NODES || buf.len() > MAX_LOG_OUT_BYTES {
					break;
				}
			}
			if vals.len() > limit {
				let _ = write!(buf, ", ...+{}", vals.len() - limit);
			}
			buf.push(']');
		}
		Value::Record { val, .. } => {
			buf.push('{');
			let limit = val.len().min(MAX_LOG_RECORD);
			for (i, (k, v)) in val.iter().take(limit).enumerate() {
				if i > 0 {
					buf.push_str(", ");
				}
				let (key, key_trunc) = trunc_utf8(k, MAX_LOG_STRING);
				buf.push_str(key);
				if key_trunc {
					buf.push_str("...");
				}
				buf.push_str(": ");
				summarize_inner(v, buf, nodes);
				if *nodes > MAX_LOG_NODES || buf.len() > MAX_LOG_OUT_BYTES {
					break;
				}
			}
			if val.len() > limit {
				let _ = write!(buf, ", ...+{}", val.len() - limit);
			}
			buf.push('}');
		}
		Value::Error { error, .. } => {
			let _ = write!(buf, "<error: {error}>");
		}
		other => {
			let _ = write!(buf, "<{}>", other.get_type());
		}
	}
}

#[cfg(test)]
mod tests;
