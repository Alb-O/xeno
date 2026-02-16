/// Nu interop for typed effect decoding.
///
/// Runtime decode surfaces:
/// * Macro execution: `Nothing`, effect `Record`, effect `List`, or batch envelope.
/// * Hook execution: same as macro, but `stop` effects are allowed.
///
/// Config decode surface:
/// * keybinding custom values decode through [`decode_single_dispatch_effect`].
use std::fmt::Write;

use xeno_nu_data::{Record, Value};

use crate::{Invocation, schema};

const EFFECT_FIELD_TYPE: &str = "type";
const EFFECT_FIELD_LEVEL: &str = "level";
const EFFECT_FIELD_MESSAGE: &str = "message";
const EFFECT_FIELD_EFFECTS: &str = "effects";
const EFFECT_FIELD_SCHEMA_VERSION: &str = "schema_version";

const EFFECT_TYPE_DISPATCH: &str = "dispatch";
const EFFECT_TYPE_NOTIFY: &str = "notify";
const EFFECT_TYPE_STOP: &str = "stop";

const DEFAULT_SCHEMA_VERSION: i64 = 1;

/// Safety budgets for decoding Nu macro/hook return values.
#[derive(Debug, Clone, Copy)]
pub struct DecodeBudget {
	pub max_effects: usize,
	pub max_string_len: usize,
	pub max_args: usize,
	pub max_action_count: usize,
	/// Maximum number of Value nodes visited during decode.
	pub max_nodes: usize,
}

impl DecodeBudget {
	pub const fn macro_defaults() -> Self {
		Self {
			max_effects: schema::DEFAULT_LIMITS.max_invocations,
			max_string_len: schema::DEFAULT_LIMITS.max_string_len,
			max_args: schema::DEFAULT_LIMITS.max_args,
			max_action_count: schema::DEFAULT_LIMITS.max_action_count,
			max_nodes: 50_000,
		}
	}

	pub const fn hook_defaults() -> Self {
		Self {
			max_effects: 32,
			max_nodes: 5_000,
			..Self::macro_defaults()
		}
	}
}

/// Nu effect notification level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NuNotifyLevel {
	Debug,
	Info,
	Warn,
	Error,
	Success,
}

impl NuNotifyLevel {
	pub fn parse(input: &str) -> Option<Self> {
		match input {
			"debug" => Some(Self::Debug),
			"info" => Some(Self::Info),
			"warn" => Some(Self::Warn),
			"error" => Some(Self::Error),
			"success" => Some(Self::Success),
			_ => None,
		}
	}

	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Debug => "debug",
			Self::Info => "info",
			Self::Warn => "warn",
			Self::Error => "error",
			Self::Success => "success",
		}
	}
}

/// Typed effect produced by Nu runtime execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NuEffect {
	/// Dispatch a canonical invocation.
	Dispatch(Invocation),
	/// Emit a host notification.
	Notify { level: NuNotifyLevel, message: String },
	/// Stop downstream hook processing.
	StopPropagation,
}

/// Decoded batch of effects plus envelope metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuEffectBatch {
	pub schema_version: i64,
	pub effects: Vec<NuEffect>,
	pub warnings: Vec<String>,
}

impl Default for NuEffectBatch {
	fn default() -> Self {
		Self {
			schema_version: DEFAULT_SCHEMA_VERSION,
			effects: Vec::new(),
			warnings: Vec::new(),
		}
	}
}

impl NuEffectBatch {
	pub fn into_dispatches(self) -> Vec<Invocation> {
		self.effects
			.into_iter()
			.filter_map(|effect| match effect {
				NuEffect::Dispatch(invocation) => Some(invocation),
				NuEffect::Notify { .. } | NuEffect::StopPropagation => None,
			})
			.collect()
	}

	pub fn has_stop_propagation(&self) -> bool {
		self.effects.iter().any(|effect| matches!(effect, NuEffect::StopPropagation))
	}
}

/// Capability tokens for Nu-produced effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NuCapability {
	DispatchAction,
	DispatchCommand,
	DispatchEditorCommand,
	DispatchMacro,
	Notify,
	StopPropagation,
	ReadContext,
}

impl NuCapability {
	pub fn parse(input: &str) -> Option<Self> {
		match input {
			"dispatch_action" => Some(Self::DispatchAction),
			"dispatch_command" => Some(Self::DispatchCommand),
			"dispatch_editor_command" => Some(Self::DispatchEditorCommand),
			"dispatch_macro" => Some(Self::DispatchMacro),
			"notify" => Some(Self::Notify),
			"stop_propagation" => Some(Self::StopPropagation),
			"read_context" => Some(Self::ReadContext),
			_ => None,
		}
	}

	pub const fn as_str(self) -> &'static str {
		match self {
			Self::DispatchAction => "dispatch_action",
			Self::DispatchCommand => "dispatch_command",
			Self::DispatchEditorCommand => "dispatch_editor_command",
			Self::DispatchMacro => "dispatch_macro",
			Self::Notify => "notify",
			Self::StopPropagation => "stop_propagation",
			Self::ReadContext => "read_context",
		}
	}
}

pub fn required_capability_for_effect(effect: &NuEffect) -> NuCapability {
	match effect {
		NuEffect::Dispatch(Invocation::Action { .. }) | NuEffect::Dispatch(Invocation::ActionWithChar { .. }) => NuCapability::DispatchAction,
		NuEffect::Dispatch(Invocation::Command { .. }) => NuCapability::DispatchCommand,
		NuEffect::Dispatch(Invocation::EditorCommand { .. }) => NuCapability::DispatchEditorCommand,
		NuEffect::Dispatch(Invocation::Nu { .. }) => NuCapability::DispatchMacro,
		NuEffect::Notify { .. } => NuCapability::Notify,
		NuEffect::StopPropagation => NuCapability::StopPropagation,
	}
}

/// Decode macro return values into typed effects.
pub fn decode_macro_effects_with_budget(value: Value, budget: DecodeBudget) -> Result<NuEffectBatch, String> {
	decode_effects_with_budget(value, budget, DecodeSurface::Macro)
}

/// Decode hook return values into typed effects.
pub fn decode_hook_effects_with_budget(value: Value, budget: DecodeBudget) -> Result<NuEffectBatch, String> {
	decode_effects_with_budget(value, budget, DecodeSurface::Hook)
}

pub fn decode_macro_effects(value: Value) -> Result<NuEffectBatch, String> {
	decode_macro_effects_with_budget(value, DecodeBudget::macro_defaults())
}

pub fn decode_hook_effects(value: Value) -> Result<NuEffectBatch, String> {
	decode_hook_effects_with_budget(value, DecodeBudget::hook_defaults())
}

/// Decode a single dispatch effect into an [`Invocation`].
pub fn decode_single_dispatch_effect(value: &Value, field_path: &str) -> Result<Invocation, String> {
	match value {
		Value::Record { val, .. } => {
			let budget = DecodeBudget::macro_defaults();
			let mut state = DecodeState::new(DecodeSurface::Macro);
			state.path.segments.clear();
			state.path.segments.push(PathSeg::RootLabel(field_path));
			let effect = decode_effect_record(val, &budget, &mut state)?;
			match effect {
				NuEffect::Dispatch(invocation) => Ok(invocation),
				NuEffect::Notify { .. } | NuEffect::StopPropagation => Err(format!("Nu decode error at {field_path}: expected dispatch effect record")),
			}
		}
		other => Err(format!("Nu decode error at {field_path}: expected effect record, got {}", other.get_type())),
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecodeSurface {
	Macro,
	Hook,
}

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

struct DecodeState<'a> {
	path: DecodePath<'a>,
	surface: DecodeSurface,
	nodes_visited: usize,
	batch: NuEffectBatch,
}

impl<'a> DecodeState<'a> {
	fn new(surface: DecodeSurface) -> Self {
		Self {
			path: DecodePath::new(),
			surface,
			nodes_visited: 0,
			batch: NuEffectBatch::default(),
		}
	}

	fn visit_node(&mut self, budget: &DecodeBudget) -> Result<(), String> {
		self.nodes_visited += 1;
		if self.nodes_visited > budget.max_nodes {
			Err(format!(
				"Nu decode error at {}: value traversal exceeds {} nodes",
				self.path.format(),
				budget.max_nodes
			))
		} else {
			Ok(())
		}
	}

	fn err(&self, msg: impl std::fmt::Display) -> String {
		format!("Nu decode error at {}: {msg}", self.path.format())
	}

	fn push_effect(&mut self, effect: NuEffect, budget: &DecodeBudget) -> Result<(), String> {
		if self.batch.effects.len() >= budget.max_effects {
			return Err(self.err(format_args!("effect count exceeds {}", budget.max_effects)));
		}
		self.batch.effects.push(effect);
		Ok(())
	}
}

fn decode_effects_with_budget(value: Value, budget: DecodeBudget, surface: DecodeSurface) -> Result<NuEffectBatch, String> {
	let mut state = DecodeState::new(surface);
	decode_runtime_value(value, &budget, &mut state)?;
	Ok(state.batch)
}

fn decode_runtime_value(value: Value, budget: &DecodeBudget, state: &mut DecodeState<'_>) -> Result<(), String> {
	state.visit_node(budget)?;
	match value {
		Value::Nothing { .. } => Ok(()),
		Value::Record { ref val, .. } => decode_root_record_or_effect(val, budget, state),
		Value::List { vals, .. } => decode_effect_list(vals, budget, state),
		Value::String { .. } => {
			Err(state.err("string returns are not supported; return typed effects via built-ins: xeno effect, xeno effects normalize, xeno call"))
		}
		other => Err(state.err(format_args!("expected effect record/list/nothing, got {}", other.get_type()))),
	}
}

fn decode_root_record_or_effect(record: &Record, budget: &DecodeBudget, state: &mut DecodeState<'_>) -> Result<(), String> {
	if record.contains(EFFECT_FIELD_EFFECTS) {
		state.path.push_field(EFFECT_FIELD_SCHEMA_VERSION);
		let schema_version = match record.get(EFFECT_FIELD_SCHEMA_VERSION) {
			None => DEFAULT_SCHEMA_VERSION,
			Some(Value::Int { val, .. }) => *val,
			Some(other) => {
				let err = state.err(format_args!("expected int, got {}", other.get_type()));
				state.path.pop();
				return Err(err);
			}
		};
		state.path.pop();
		state.batch.schema_version = schema_version.max(1);

		state.path.push_field(EFFECT_FIELD_EFFECTS);
		let effects = match record.get(EFFECT_FIELD_EFFECTS) {
			Some(Value::List { vals, .. }) => vals.clone(),
			Some(other) => {
				let err = state.err(format_args!("expected list<record>, got {}", other.get_type()));
				state.path.pop();
				return Err(err);
			}
			None => {
				let err = state.err("missing required field");
				state.path.pop();
				return Err(err);
			}
		};
		let result = decode_effect_list(effects, budget, state);
		state.path.pop();
		result
	} else {
		let effect = decode_effect_record(record, budget, state)?;
		state.push_effect(effect, budget)
	}
}

fn decode_effect_list(values: Vec<Value>, budget: &DecodeBudget, state: &mut DecodeState<'_>) -> Result<(), String> {
	for (idx, item) in values.into_iter().enumerate() {
		state.path.push_index(idx);
		state.visit_node(budget)?;
		match item {
			Value::Nothing { .. } => {}
			Value::Record { ref val, .. } => {
				let effect = decode_effect_record(val, budget, state)?;
				state.push_effect(effect, budget)?;
			}
			other => {
				let err = state.err(format_args!("expected effect record or nothing, got {}", other.get_type()));
				state.path.pop();
				return Err(err);
			}
		}
		state.path.pop();
	}
	Ok(())
}

fn decode_effect_record(record: &Record, budget: &DecodeBudget, state: &mut DecodeState<'_>) -> Result<NuEffect, String> {
	if record.contains(schema::KIND) && !record.contains(EFFECT_FIELD_TYPE) {
		return Err(state.err("legacy invocation records are no longer accepted; return a typed effect record with `type: \"dispatch\"`"));
	}

	let effect_type = required_string_field(record, EFFECT_FIELD_TYPE, budget, state)?;
	match effect_type.as_str() {
		EFFECT_TYPE_DISPATCH => Ok(NuEffect::Dispatch(decode_dispatch_invocation(record, budget, state)?)),
		EFFECT_TYPE_NOTIFY => {
			let level_raw = required_string_field(record, EFFECT_FIELD_LEVEL, budget, state)?;
			let Some(level) = NuNotifyLevel::parse(&level_raw) else {
				return Err(state.err(format_args!("unknown notify level '{level_raw}'")));
			};
			let message = required_string_field(record, EFFECT_FIELD_MESSAGE, budget, state)?;
			Ok(NuEffect::Notify { level, message })
		}
		EFFECT_TYPE_STOP => {
			if state.surface != DecodeSurface::Hook {
				return Err(state.err("stop effect is only allowed in hook execution"));
			}
			Ok(NuEffect::StopPropagation)
		}
		other => Err(state.err(format_args!("unknown effect type '{other}'"))),
	}
}

fn decode_dispatch_invocation(record: &Record, budget: &DecodeBudget, state: &mut DecodeState<'_>) -> Result<Invocation, String> {
	let kind = required_string_field(record, schema::KIND, budget, state)?;
	let name = required_string_field(record, schema::NAME, budget, state)?;

	match kind.as_str() {
		schema::KIND_ACTION => {
			let count = optional_int_field(record, schema::COUNT, budget, state)?.unwrap_or(1).max(1);
			let extend = optional_bool_field(record, schema::EXTEND, state)?.unwrap_or(false);
			let register = optional_char_field(record, schema::REGISTER, budget, state)?;
			let char_arg = optional_char_field(record, schema::CHAR, budget, state)?;

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
		schema::KIND_COMMAND => Ok(Invocation::Command {
			name,
			args: optional_string_list_field(record, schema::ARGS, budget, state)?.unwrap_or_default(),
		}),
		schema::KIND_EDITOR => Ok(Invocation::EditorCommand {
			name,
			args: optional_string_list_field(record, schema::ARGS, budget, state)?.unwrap_or_default(),
		}),
		schema::KIND_NU => Ok(Invocation::Nu {
			name,
			args: optional_string_list_field(record, schema::ARGS, budget, state)?.unwrap_or_default(),
		}),
		other => Err(state.err(format_args!("unknown invocation kind '{other}'"))),
	}
}

fn required_string_field<'a>(record: &Record, field: &'a str, budget: &DecodeBudget, state: &mut DecodeState<'a>) -> Result<String, String> {
	state.path.push_field(field);
	let out = match record.get(field) {
		Some(Value::String { val, .. }) => {
			validate_string_limit(state, val, budget)?;
			if val.is_empty() {
				Err(state.err("must not be empty"))
			} else {
				Ok(val.clone())
			}
		}
		Some(other) => Err(state.err(format_args!("expected string, got {}", other.get_type()))),
		None => Err(state.err("missing required field")),
	};
	state.path.pop();
	out
}

fn optional_int_field<'a>(record: &Record, field: &'a str, budget: &DecodeBudget, state: &mut DecodeState<'a>) -> Result<Option<usize>, String> {
	if !record.contains(field) {
		return Ok(None);
	}

	state.path.push_field(field);
	let out = match record.get(field) {
		Some(Value::Nothing { .. }) => Ok(None),
		Some(Value::Int { val, .. }) if *val >= 0 => {
			let as_usize = *val as usize;
			if field == schema::COUNT && as_usize > budget.max_action_count {
				Err(state.err(format_args!("exceeds max action count {}", budget.max_action_count)))
			} else {
				Ok(Some(as_usize))
			}
		}
		Some(Value::Int { .. }) => Err(state.err("must be non-negative")),
		Some(other) => Err(state.err(format_args!("expected int, got {}", other.get_type()))),
		None => Ok(None),
	};
	state.path.pop();
	out
}

fn optional_bool_field<'a>(record: &Record, field: &'a str, state: &mut DecodeState<'a>) -> Result<Option<bool>, String> {
	if !record.contains(field) {
		return Ok(None);
	}

	state.path.push_field(field);
	let out = match record.get(field) {
		Some(Value::Nothing { .. }) => Ok(None),
		Some(Value::Bool { val, .. }) => Ok(Some(*val)),
		Some(other) => Err(state.err(format_args!("expected bool, got {}", other.get_type()))),
		None => Ok(None),
	};
	state.path.pop();
	out
}

fn optional_char_field<'a>(record: &Record, field: &'a str, budget: &DecodeBudget, state: &mut DecodeState<'a>) -> Result<Option<char>, String> {
	if !record.contains(field) {
		return Ok(None);
	}

	state.path.push_field(field);
	let out = match record.get(field) {
		Some(Value::Nothing { .. }) => Ok(None),
		Some(Value::String { val, .. }) => {
			validate_string_limit(state, val, budget)?;
			let mut chars = val.chars();
			let Some(ch) = chars.next() else {
				state.path.pop();
				return Err(state.err("must be exactly one character"));
			};
			if chars.next().is_some() {
				Err(state.err("must be exactly one character"))
			} else {
				Ok(Some(ch))
			}
		}
		Some(other) => Err(state.err(format_args!("expected string, got {}", other.get_type()))),
		None => Ok(None),
	};
	state.path.pop();
	out
}

fn optional_string_list_field<'a>(record: &Record, field: &'a str, budget: &DecodeBudget, state: &mut DecodeState<'a>) -> Result<Option<Vec<String>>, String> {
	if !record.contains(field) {
		return Ok(None);
	}

	state.path.push_field(field);
	let out = match record.get(field) {
		Some(Value::Nothing { .. }) => Ok(None),
		Some(Value::List { vals, .. }) => {
			if vals.len() > budget.max_args {
				Err(state.err(format_args!("argument count exceeds {}", budget.max_args)))
			} else {
				let mut out = Vec::with_capacity(vals.len());
				for (idx, item) in vals.iter().enumerate() {
					state.path.push_index(idx);
					match item {
						Value::String { val, .. } => {
							validate_string_limit(state, val, budget)?;
							out.push(val.clone());
						}
						other => {
							let err = state.err(format_args!("expected string, got {}", other.get_type()));
							state.path.pop();
							return Err(err);
						}
					}
					state.path.pop();
				}
				Ok(Some(out))
			}
		}
		Some(other) => Err(state.err(format_args!("expected list<string>, got {}", other.get_type()))),
		None => Ok(None),
	};
	state.path.pop();
	out
}

fn validate_string_limit(state: &DecodeState<'_>, value: &str, budget: &DecodeBudget) -> Result<(), String> {
	if value.len() > budget.max_string_len {
		Err(state.err(format_args!("max string length {} exceeded", budget.max_string_len)))
	} else {
		Ok(())
	}
}

#[cfg(test)]
mod tests;
