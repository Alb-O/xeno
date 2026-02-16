/// Canonical invocation record field names and Nu Value constructors.
///
/// All invocation records share these field names. The constructors produce
/// `xeno_nu_value::Value::Record` values that are guaranteed to decode
/// correctly through [`crate::nu::decode_invocations`].

#[cfg(feature = "nu")]
use xeno_nu_value::{Record, Span, Value};

// ---------------------------------------------------------------------------
// Field name constants
// ---------------------------------------------------------------------------

pub const KIND: &str = "kind";
pub const NAME: &str = "name";
pub const ARGS: &str = "args";
pub const COUNT: &str = "count";
pub const EXTEND: &str = "extend";
pub const REGISTER: &str = "register";
pub const CHAR: &str = "char";

// Kind values
pub const KIND_ACTION: &str = "action";
pub const KIND_COMMAND: &str = "command";
pub const KIND_EDITOR: &str = "editor";
pub const KIND_NU: &str = "nu";

// ---------------------------------------------------------------------------
// Value constructors (require `nu` feature)
// ---------------------------------------------------------------------------

/// Build an action invocation record.
#[cfg(feature = "nu")]
pub fn action_record(name: String, count: i64, extend: bool, register: Option<char>, char_arg: Option<char>, span: Span) -> Value {
	let mut r = Record::new();
	r.push(KIND, Value::string(KIND_ACTION, span));
	r.push(NAME, Value::string(name, span));
	r.push(COUNT, Value::int(count, span));
	r.push(EXTEND, Value::bool(extend, span));
	r.push(REGISTER, register.map_or_else(|| Value::nothing(span), |c| Value::string(c.to_string(), span)));
	if let Some(ch) = char_arg {
		r.push(CHAR, Value::string(ch.to_string(), span));
	}
	Value::record(r, span)
}

/// Build a command/editor/nu invocation record.
#[cfg(feature = "nu")]
pub fn invocation_record(kind: &str, name: String, args: Vec<String>, span: Span) -> Value {
	let mut r = Record::new();
	r.push(KIND, Value::string(kind, span));
	r.push(NAME, Value::string(name, span));
	r.push(ARGS, Value::list(args.into_iter().map(|a| Value::string(a, span)).collect(), span));
	Value::record(r, span)
}

/// Build a command invocation record.
#[cfg(feature = "nu")]
pub fn command_record(name: String, args: Vec<String>, span: Span) -> Value {
	invocation_record(KIND_COMMAND, name, args, span)
}

/// Build an editor command invocation record.
#[cfg(feature = "nu")]
pub fn editor_record(name: String, args: Vec<String>, span: Span) -> Value {
	invocation_record(KIND_EDITOR, name, args, span)
}

/// Build a Nu macro invocation record.
#[cfg(feature = "nu")]
pub fn nu_record(name: String, args: Vec<String>, span: Span) -> Value {
	invocation_record(KIND_NU, name, args, span)
}

// ---------------------------------------------------------------------------
// Shared invocation limits
// ---------------------------------------------------------------------------

/// Validation limits for invocation records, shared by emit commands and decoder.
pub struct InvocationLimits {
	/// Max invocations in a batch (emit-many, decode).
	pub max_invocations: usize,
	/// Max positional args per invocation.
	pub max_args: usize,
	/// Max string length for name/args.
	pub max_string_len: usize,
	/// Max action repeat count.
	pub max_action_count: usize,
}

/// Default limits shared across emit, emit-many, and macro decode.
pub const DEFAULT_LIMITS: InvocationLimits = InvocationLimits {
	max_invocations: 256,
	max_args: 64,
	max_string_len: 4096,
	max_action_count: 10_000,
};

// ---------------------------------------------------------------------------
// Shared record validator (require `nu` feature)
// ---------------------------------------------------------------------------

/// Validate and normalize an invocation record into canonical form.
///
/// `idx` is used for error paths: `Some(7)` produces `items[7].field`.
#[cfg(feature = "nu")]
pub fn validate_invocation_record(rec: &Record, idx: Option<usize>, limits: &InvocationLimits, span: Span) -> Result<Value, String> {
	let kind = val_required_str(rec, KIND, idx, limits)?;
	let name = val_required_str(rec, NAME, idx, limits)?;
	if name.is_empty() {
		return Err(val_err(idx, NAME, "must not be empty"));
	}

	match kind.as_str() {
		KIND_ACTION => {
			let count = val_optional_int(rec, COUNT, idx)?.map(|c| c.max(1)).unwrap_or(1);
			let extend = val_optional_bool(rec, EXTEND, idx)?.unwrap_or(false);
			let register = val_optional_char(rec, REGISTER, idx)?;
			let char_arg = val_optional_char(rec, CHAR, idx)?;
			Ok(action_record(name, count, extend, register, char_arg, span))
		}
		KIND_COMMAND | KIND_EDITOR | KIND_NU => {
			let args = val_optional_string_list(rec, ARGS, idx, limits)?.unwrap_or_default();
			match kind.as_str() {
				KIND_COMMAND => Ok(command_record(name, args, span)),
				KIND_EDITOR => Ok(editor_record(name, args, span)),
				_ => Ok(nu_record(name, args, span)),
			}
		}
		other => Err(val_err(idx, KIND, &format!("unknown kind '{other}'"))),
	}
}

fn val_err(idx: Option<usize>, field: &str, msg: &str) -> String {
	match idx {
		Some(i) => format!("items[{i}].{field}: {msg}"),
		None => format!("{field}: {msg}"),
	}
}

#[cfg(feature = "nu")]
fn val_required_str(rec: &Record, field: &str, idx: Option<usize>, limits: &InvocationLimits) -> Result<String, String> {
	let val = rec.get(field).ok_or_else(|| val_err(idx, field, "missing required field"))?;
	match val {
		Value::String { val, .. } => {
			if val.len() > limits.max_string_len {
				return Err(val_err(idx, field, &format!("exceeds {} bytes", limits.max_string_len)));
			}
			Ok(val.clone())
		}
		other => Err(val_err(idx, field, &format!("expected string, got {}", other.get_type()))),
	}
}

#[cfg(feature = "nu")]
fn val_optional_int(rec: &Record, field: &str, idx: Option<usize>) -> Result<Option<i64>, String> {
	let Some(val) = rec.get(field) else { return Ok(None) };
	match val {
		Value::Nothing { .. } => Ok(None),
		Value::Int { val, .. } => Ok(Some(*val)),
		other => Err(val_err(idx, field, &format!("expected int, got {}", other.get_type()))),
	}
}

#[cfg(feature = "nu")]
fn val_optional_bool(rec: &Record, field: &str, idx: Option<usize>) -> Result<Option<bool>, String> {
	let Some(val) = rec.get(field) else { return Ok(None) };
	match val {
		Value::Nothing { .. } => Ok(None),
		Value::Bool { val, .. } => Ok(Some(*val)),
		other => Err(val_err(idx, field, &format!("expected bool, got {}", other.get_type()))),
	}
}

#[cfg(feature = "nu")]
fn val_optional_char(rec: &Record, field: &str, idx: Option<usize>) -> Result<Option<char>, String> {
	let Some(val) = rec.get(field) else { return Ok(None) };
	let s = match val {
		Value::Nothing { .. } => return Ok(None),
		Value::String { val, .. } => val,
		other => return Err(val_err(idx, field, &format!("expected string, got {}", other.get_type()))),
	};
	let mut chars = s.chars();
	let Some(ch) = chars.next() else {
		return Err(val_err(idx, field, "must be exactly one character"));
	};
	if chars.next().is_some() {
		return Err(val_err(idx, field, "must be exactly one character"));
	}
	Ok(Some(ch))
}

#[cfg(feature = "nu")]
fn val_optional_string_list(rec: &Record, field: &str, idx: Option<usize>, limits: &InvocationLimits) -> Result<Option<Vec<String>>, String> {
	let Some(val) = rec.get(field) else { return Ok(None) };
	let list = match val {
		Value::Nothing { .. } => return Ok(None),
		Value::List { vals, .. } => vals,
		other => return Err(val_err(idx, field, &format!("expected list<string>, got {}", other.get_type()))),
	};
	if list.len() > limits.max_args {
		return Err(val_err(idx, field, &format!("exceeds {} args", limits.max_args)));
	}
	let mut out = Vec::with_capacity(list.len());
	for (i, item) in list.iter().enumerate() {
		match item {
			Value::String { val, .. } => {
				if val.len() > limits.max_string_len {
					return Err(val_err(idx, &format!("{field}[{i}]"), &format!("exceeds {} bytes", limits.max_string_len)));
				}
				out.push(val.clone());
			}
			other => return Err(val_err(idx, &format!("{field}[{i}]"), &format!("expected string, got {}", other.get_type()))),
		}
	}
	Ok(Some(out))
}
