//! Decoding of Nu return values into [`Invocation`] types.
//!
//! Handles structured records (`{ kind: "action", name: "move_right" }`)
//! and lists of records. Applies size/depth/node limits to guard against
//! runaway macros.
//!
//! String return values are not supported — use the built-in prelude
//! constructors (`action`, `command`, `editor`, `"nu run"`) instead.

use std::fmt::Write;

use nu_protocol::Value;

use super::Invocation;

/// Safety limits for decoding Nu macro/hook return values.
#[derive(Debug, Clone, Copy)]
pub struct DecodeLimits {
	pub max_invocations: usize,
	pub max_depth: usize,
	pub max_string_len: usize,
	pub max_args: usize,
	pub max_action_count: usize,
	/// Maximum number of Value nodes visited during decode. Guards against
	/// huge return values (e.g., giant lists of nothing) burning CPU.
	pub max_nodes: usize,
}

impl DecodeLimits {
	pub const fn macro_defaults() -> Self {
		Self {
			max_invocations: 256,
			max_depth: 8,
			max_string_len: 4096,
			max_args: 64,
			max_action_count: 10_000,
			max_nodes: 50_000,
		}
	}

	pub const fn hook_defaults() -> Self {
		Self {
			max_invocations: 32,
			max_nodes: 5_000,
			..Self::macro_defaults()
		}
	}
}

// --- Path tracking (zero-alloc on happy path) ---

#[derive(Clone, Copy)]
enum PathSeg<'a> {
	Root,
	RootLabel(&'a str),
	Index(usize),
	Field(&'a str),
}

struct DecodePath<'a> {
	segments: Vec<PathSeg<'a>>,
}

impl<'a> DecodePath<'a> {
	fn new() -> Self {
		Self { segments: vec![PathSeg::Root] }
	}

	fn push_index(&mut self, idx: usize) {
		self.segments.push(PathSeg::Index(idx));
	}

	fn push_field(&mut self, field: &'a str) {
		self.segments.push(PathSeg::Field(field));
	}

	fn pop(&mut self) {
		self.segments.pop();
	}

	/// Format the path into a string. Only called on error.
	fn format(&self) -> String {
		let mut out = String::new();
		for seg in &self.segments {
			match seg {
				PathSeg::Root => out.push_str("return"),
				PathSeg::RootLabel(label) => out.push_str(label),
				PathSeg::Index(idx) => write!(out, "[{idx}]").unwrap(),
				PathSeg::Field(name) => write!(out, ".{name}").unwrap(),
			}
		}
		out
	}
}

// --- Decode state ---

struct DecodeState<'a> {
	path: DecodePath<'a>,
	nodes_visited: usize,
	invocations: Vec<Invocation>,
}

impl<'a> DecodeState<'a> {
	fn new() -> Self {
		Self {
			path: DecodePath::new(),
			nodes_visited: 0,
			invocations: Vec::new(),
		}
	}

	fn visit_node(&mut self, limits: &DecodeLimits) -> Result<(), String> {
		self.nodes_visited += 1;
		if self.nodes_visited > limits.max_nodes {
			Err(format!(
				"Nu decode error at {}: value traversal exceeds {} nodes",
				self.path.format(),
				limits.max_nodes
			))
		} else {
			Ok(())
		}
	}

	fn err(&self, msg: impl std::fmt::Display) -> String {
		format!("Nu decode error at {}: {msg}", self.path.format())
	}

	fn push_invocation(&mut self, invocation: Invocation, limits: &DecodeLimits) -> Result<(), String> {
		if self.invocations.len() >= limits.max_invocations {
			return Err(self.err(format_args!("invocation count exceeds {}", limits.max_invocations)));
		}
		self.invocations.push(invocation);
		Ok(())
	}
}

// --- Public API ---

/// Decode invocation return values from Nu macros and hooks.
pub fn decode_invocations(value: Value) -> Result<Vec<Invocation>, String> {
	decode_invocations_with_limits(value, DecodeLimits::macro_defaults())
}

/// Decode invocation return values from Nu macros and hooks with explicit limits.
pub fn decode_invocations_with_limits(value: Value, limits: DecodeLimits) -> Result<Vec<Invocation>, String> {
	let mut state = DecodeState::new();
	decode_value(value, &limits, 0, &mut state)?;
	Ok(state.invocations)
}

/// Decode a single Nu record value into an [`Invocation`].
///
/// Used for config key bindings where each value must be exactly one invocation record.
/// Returns an error if the value is not a record with a `kind` field.
pub fn decode_single_invocation(value: &Value, field_path: &str) -> Result<Invocation, String> {
	match value {
		Value::Record { val, .. } => {
			let limits = DecodeLimits::macro_defaults();
			let mut state = DecodeState::new();
			state.path.segments.clear();
			state.path.segments.push(PathSeg::RootLabel(field_path));
			decode_invocation_record(val, &limits, &mut state)?;
			state
				.invocations
				.into_iter()
				.next()
				.ok_or_else(|| format!("Nu decode error at {field_path}: decoded zero invocations"))
		}
		other => Err(format!("Nu decode error at {field_path}: expected invocation record, got {}", other.get_type())),
	}
}

// --- Core decode ---

fn decode_value(value: Value, limits: &DecodeLimits, depth: usize, state: &mut DecodeState<'_>) -> Result<(), String> {
	state.visit_node(limits)?;

	if depth > limits.max_depth {
		return Err(state.err(format_args!("nesting depth exceeds {}", limits.max_depth)));
	}

	match value {
		Value::Nothing { .. } => Ok(()),
		Value::String { .. } => Err(state.err("string returns are no longer supported; use prelude constructors: action, command, editor, \"nu run\"")),
		Value::List { vals, .. } => {
			for (idx, value) in vals.into_iter().enumerate() {
				state.path.push_index(idx);
				decode_value(value, limits, depth + 1, state)?;
				state.path.pop();
			}
			Ok(())
		}
		Value::Record { val, .. } => decode_invocation_record(&val, limits, state),
		other => Err(state.err(format_args!("expected record/list/nothing, got {}", other.get_type()))),
	}
}

fn decode_invocation_record(record: &nu_protocol::Record, limits: &DecodeLimits, state: &mut DecodeState<'_>) -> Result<(), String> {
	if !record.contains("kind") {
		return Err(state.err("record must include 'kind'"));
	}
	let invocation = decode_structured_invocation(record, limits, state)?;
	validate_invocation_limits(&invocation, state, limits)?;
	state.push_invocation(invocation, limits)
}

fn decode_structured_invocation(record: &nu_protocol::Record, limits: &DecodeLimits, state: &mut DecodeState<'_>) -> Result<Invocation, String> {
	let kind = required_string_field(record, "kind", limits, state)?;
	let name = required_string_field(record, "name", limits, state)?;

	match kind.as_str() {
		"action" => {
			let count = optional_int_field(record, "count", limits, state)?.unwrap_or(1).max(1);
			let extend = optional_bool_field(record, "extend", state)?.unwrap_or(false);
			let register = optional_char_field(record, "register", limits, state)?;
			let char_arg = optional_char_field(record, "char", limits, state)?;

			if let Some(char_arg) = char_arg {
				Ok(Invocation::ActionWithChar {
					name,
					count,
					extend,
					register,
					char_arg,
				})
			} else {
				Ok(Invocation::Action { name, count, extend, register })
			}
		}
		"command" => Ok(Invocation::Command {
			name,
			args: optional_string_list_field(record, "args", limits, state)?.unwrap_or_default(),
		}),
		"editor" => Ok(Invocation::EditorCommand {
			name,
			args: optional_string_list_field(record, "args", limits, state)?.unwrap_or_default(),
		}),
		"nu" => Ok(Invocation::Nu {
			name,
			args: optional_string_list_field(record, "args", limits, state)?.unwrap_or_default(),
		}),
		other => Err(state.err(format_args!("unknown invocation kind '{other}'"))),
	}
}

// --- Field helpers ---

fn required_string_field<'a>(record: &nu_protocol::Record, field: &'a str, limits: &DecodeLimits, state: &mut DecodeState<'a>) -> Result<String, String> {
	let value = record.get(field).ok_or_else(|| state.err(format_args!("missing required field '{field}'")))?;
	match value {
		Value::String { val, .. } => {
			state.path.push_field(field);
			let result = validate_string_limit(state, val, limits);
			state.path.pop();
			result?;
			Ok(val.clone())
		}
		other => {
			state.path.push_field(field);
			let err = state.err(format_args!("must be string, got {}", other.get_type()));
			state.path.pop();
			Err(err)
		}
	}
}

fn optional_bool_field<'a>(record: &nu_protocol::Record, field: &'a str, state: &mut DecodeState<'a>) -> Result<Option<bool>, String> {
	let Some(value) = record.get(field) else {
		return Ok(None);
	};
	match value {
		Value::Nothing { .. } => Ok(None),
		Value::Bool { val, .. } => Ok(Some(*val)),
		other => {
			state.path.push_field(field);
			let err = state.err(format_args!("must be bool, got {}", other.get_type()));
			state.path.pop();
			Err(err)
		}
	}
}

fn optional_int_field<'a>(record: &nu_protocol::Record, field: &'a str, limits: &DecodeLimits, state: &mut DecodeState<'a>) -> Result<Option<usize>, String> {
	let Some(value) = record.get(field) else {
		return Ok(None);
	};
	match value {
		Value::Nothing { .. } => Ok(None),
		Value::Int { val, .. } => {
			if *val <= 0 {
				Ok(Some(1))
			} else {
				let max = limits.max_action_count as i128;
				let clamped = (*val as i128).min(max) as usize;
				Ok(Some(clamped))
			}
		}
		other => {
			state.path.push_field(field);
			let err = state.err(format_args!("must be int, got {}", other.get_type()));
			state.path.pop();
			Err(err)
		}
	}
}

fn optional_char_field<'a>(record: &nu_protocol::Record, field: &'a str, limits: &DecodeLimits, state: &mut DecodeState<'a>) -> Result<Option<char>, String> {
	let Some(value) = record.get(field) else {
		return Ok(None);
	};
	let s = match value {
		Value::Nothing { .. } => return Ok(None),
		Value::String { val, .. } => {
			state.path.push_field(field);
			let result = validate_string_limit(state, val, limits);
			state.path.pop();
			result?;
			val
		}
		other => {
			state.path.push_field(field);
			let err = state.err(format_args!("must be single-character string, got {}", other.get_type()));
			state.path.pop();
			return Err(err);
		}
	};
	let mut chars = s.chars();
	let Some(ch) = chars.next() else {
		state.path.push_field(field);
		let err = state.err("must be exactly one character");
		state.path.pop();
		return Err(err);
	};
	if chars.next().is_some() {
		state.path.push_field(field);
		let err = state.err("must be exactly one character");
		state.path.pop();
		return Err(err);
	}
	Ok(Some(ch))
}

fn optional_string_list_field<'a>(
	record: &nu_protocol::Record,
	field: &'a str,
	limits: &DecodeLimits,
	state: &mut DecodeState<'a>,
) -> Result<Option<Vec<String>>, String> {
	let Some(value) = record.get(field) else {
		return Ok(None);
	};
	let list = match value {
		Value::Nothing { .. } => return Ok(None),
		Value::List { vals, .. } => vals,
		other => {
			state.path.push_field(field);
			let err = state.err(format_args!("must be list<string>, got {}", other.get_type()));
			state.path.pop();
			return Err(err);
		}
	};
	if list.len() > limits.max_args {
		state.path.push_field(field);
		let err = state.err(format_args!("exceeds {} args", limits.max_args));
		state.path.pop();
		return Err(err);
	}

	let mut out = Vec::with_capacity(list.len());
	for (idx, item) in list.iter().enumerate() {
		match item {
			Value::String { val, .. } => {
				state.path.push_field(field);
				state.path.push_index(idx);
				let result = validate_string_limit(state, val, limits);
				state.path.pop();
				state.path.pop();
				result?;
				out.push(val.clone());
			}
			other => {
				state.path.push_field(field);
				state.path.push_index(idx);
				let err = state.err(format_args!("must be string, got {}", other.get_type()));
				state.path.pop();
				state.path.pop();
				return Err(err);
			}
		}
	}

	Ok(Some(out))
}

fn validate_string_limit(state: &DecodeState<'_>, value: &str, limits: &DecodeLimits) -> Result<(), String> {
	if value.len() > limits.max_string_len {
		return Err(state.err(format_args!("exceeds max string length {}", limits.max_string_len)));
	}
	Ok(())
}

fn validate_invocation_limits(invocation: &Invocation, state: &mut DecodeState<'_>, limits: &DecodeLimits) -> Result<(), String> {
	match invocation {
		Invocation::Action { name, count, .. } | Invocation::ActionWithChar { name, count, .. } => {
			state.path.push_field("name");
			let result = validate_string_limit(state, name, limits);
			state.path.pop();
			result?;
			if *count > limits.max_action_count {
				state.path.push_field("count");
				let err = state.err(format_args!("action count exceeds {}", limits.max_action_count));
				state.path.pop();
				return Err(err);
			}
		}
		Invocation::Command { name, args } | Invocation::EditorCommand { name, args } | Invocation::Nu { name, args } => {
			state.path.push_field("name");
			let result = validate_string_limit(state, name, limits);
			state.path.pop();
			result?;
			if args.len() > limits.max_args {
				state.path.push_field("args");
				let err = state.err(format_args!("exceeds {}", limits.max_args));
				state.path.pop();
				return Err(err);
			}
			for (idx, arg) in args.iter().enumerate() {
				state.path.push_field("args");
				state.path.push_index(idx);
				let result = validate_string_limit(state, arg, limits);
				state.path.pop();
				state.path.pop();
				result?;
			}
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests;
