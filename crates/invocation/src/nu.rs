/// Nu interop for typed effect decoding.
///
/// All runtime decode surfaces accept: `Nothing`, bare effect `Record`,
/// effect `List` (potentially nested), or batch envelope `Record`.
/// The `stop` effect is only allowed on the [`DecodeSurface::Hook`] surface.
///
/// Config decode surface:
/// * keybinding custom values decode through [`decode_single_dispatch_effect`].
use std::fmt::Write;

use xeno_nu_data::{Record, Value};

use crate::{CommandInvocation, CommandRoute, Invocation, schema};

const EFFECT_FIELD_TYPE: &str = "type";
const EFFECT_FIELD_LEVEL: &str = "level";
const EFFECT_FIELD_MESSAGE: &str = "message";
const EFFECT_FIELD_EFFECTS: &str = "effects";
const EFFECT_FIELD_SCHEMA_VERSION: &str = "schema_version";
const EFFECT_FIELD_WARNINGS: &str = "warnings";

const EFFECT_TYPE_DISPATCH: &str = "dispatch";
const EFFECT_TYPE_NOTIFY: &str = "notify";
const EFFECT_TYPE_STOP: &str = "stop";
const EFFECT_TYPE_EDIT: &str = "edit";
const EFFECT_TYPE_CLIPBOARD: &str = "clipboard";
const EFFECT_TYPE_STATE: &str = "state";
const EFFECT_TYPE_SCHEDULE: &str = "schedule";
const EFFECT_FIELD_OP: &str = "op";
const EFFECT_FIELD_TEXT: &str = "text";
const EFFECT_FIELD_KEY: &str = "key";
const EFFECT_FIELD_VALUE: &str = "value";
const EFFECT_FIELD_DELAY_MS: &str = "delay_ms";
const EFFECT_FIELD_MACRO: &str = "macro";
const EFFECT_FIELD_ARGS: &str = "args";

/// Maximum delay for scheduled macros (1 hour).
pub const MAX_SCHEDULE_DELAY_MS: u64 = 3_600_000;

/// Canonical effect schema version supported by this host.
pub const EFFECT_SCHEMA_VERSION: i64 = 1;

/// Hard limits for Nu function call inputs (args + env values).
///
/// These are the canonical source for sandbox call validation in
/// `xeno-nu-runtime`. Derived from [`schema::DEFAULT_LIMITS`] where applicable.
#[derive(Debug, Clone, Copy)]
pub struct NuCallLimits {
	/// Max positional arguments per function call.
	pub max_args: usize,
	/// Max byte length per argument string.
	pub max_arg_len: usize,
	/// Max Value nodes traversed in env validation.
	pub max_env_nodes: usize,
	/// Max byte length per env string (keys + leaf values).
	pub max_env_string_len: usize,
}

/// Default call limits, aligned with [`schema::DEFAULT_LIMITS`].
pub const DEFAULT_CALL_LIMITS: NuCallLimits = NuCallLimits {
	max_args: schema::DEFAULT_LIMITS.max_args,
	max_arg_len: schema::DEFAULT_LIMITS.max_string_len,
	max_env_nodes: 5_000,
	max_env_string_len: schema::DEFAULT_LIMITS.max_string_len,
};

/// Maximum Value nodes visited during macro effect decode.
pub const DEFAULT_MACRO_MAX_NODES: usize = 50_000;

/// Maximum Value nodes visited during hook effect decode.
pub const DEFAULT_HOOK_MAX_NODES: usize = 5_000;

/// Maximum effects returned by a single hook evaluation.
pub const DEFAULT_HOOK_MAX_EFFECTS: usize = 32;

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
			max_nodes: DEFAULT_MACRO_MAX_NODES,
		}
	}

	pub const fn hook_defaults() -> Self {
		Self {
			max_effects: DEFAULT_HOOK_MAX_EFFECTS,
			max_nodes: DEFAULT_HOOK_MAX_NODES,
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

/// Text edit operation for buffer manipulation from Nu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NuTextEditOp {
	/// Replace the active selection (or insert at cursor if point selection).
	ReplaceSelection,
	/// Replace the current cursor line content (excluding trailing newline).
	ReplaceLine,
}

impl NuTextEditOp {
	fn parse(s: &str) -> Option<Self> {
		match s {
			"replace_selection" => Some(Self::ReplaceSelection),
			"replace_line" => Some(Self::ReplaceLine),
			_ => None,
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
	/// Directly edit buffer text.
	EditText { op: NuTextEditOp, text: String },
	/// Write text to the clipboard (yank register).
	SetClipboard { text: String },
	/// Set a key-value pair in the persistent Nu state store.
	StateSet { key: String, value: String },
	/// Remove a key from the persistent Nu state store.
	StateUnset { key: String },
	/// Schedule a macro to run after a delay, cancelling any previous schedule with the same key.
	ScheduleSet {
		key: String,
		delay_ms: u64,
		name: String,
		args: Vec<String>,
	},
	/// Cancel a pending scheduled macro by key.
	ScheduleCancel { key: String },
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
			schema_version: EFFECT_SCHEMA_VERSION,
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
				NuEffect::Notify { .. }
				| NuEffect::StopPropagation
				| NuEffect::EditText { .. }
				| NuEffect::SetClipboard { .. }
				| NuEffect::StateSet { .. }
				| NuEffect::StateUnset { .. }
				| NuEffect::ScheduleSet { .. }
				| NuEffect::ScheduleCancel { .. } => None,
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
	EditText,
	SetClipboard,
	WriteState,
	ScheduleMacro,
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
			"edit_text" => Some(Self::EditText),
			"set_clipboard" => Some(Self::SetClipboard),
			"write_state" => Some(Self::WriteState),
			"schedule_macro" => Some(Self::ScheduleMacro),
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
			Self::EditText => "edit_text",
			Self::SetClipboard => "set_clipboard",
			Self::WriteState => "write_state",
			Self::ScheduleMacro => "schedule_macro",
		}
	}
}

pub fn required_capability_for_effect(effect: &NuEffect) -> NuCapability {
	match effect {
		NuEffect::Dispatch(Invocation::Action { .. }) | NuEffect::Dispatch(Invocation::ActionWithChar { .. }) => NuCapability::DispatchAction,
		NuEffect::Dispatch(Invocation::Command(CommandInvocation {
			route: CommandRoute::Editor, ..
		})) => NuCapability::DispatchEditorCommand,
		NuEffect::Dispatch(Invocation::Command(_)) => NuCapability::DispatchCommand,
		NuEffect::Dispatch(Invocation::Nu { .. }) => NuCapability::DispatchMacro,
		NuEffect::Notify { .. } => NuCapability::Notify,
		NuEffect::StopPropagation => NuCapability::StopPropagation,
		NuEffect::EditText { .. } => NuCapability::EditText,
		NuEffect::SetClipboard { .. } => NuCapability::SetClipboard,
		NuEffect::StateSet { .. } | NuEffect::StateUnset { .. } => NuCapability::WriteState,
		NuEffect::ScheduleSet { .. } | NuEffect::ScheduleCancel { .. } => NuCapability::ScheduleMacro,
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
///
/// Accepts either a bare effect record or an envelope containing exactly
/// one dispatch effect (as produced by `xeno effect dispatch ...`).
pub fn decode_single_dispatch_effect(value: &Value, field_path: &str) -> Result<Invocation, String> {
	match value {
		Value::Record { val, .. } => {
			let budget = DecodeBudget::macro_defaults();
			let mut state = DecodeState::new(DecodeSurface::Macro);
			state.path.segments.clear();
			state.path.segments.push(PathSeg::RootLabel(field_path));

			// Check if this is an envelope record.
			if val.contains(EFFECT_FIELD_EFFECTS) {
				decode_envelope_record(val, &budget, &mut state)?;
				if state.batch.effects.len() != 1 {
					return Err(format!(
						"Nu decode error at {field_path}: expected exactly one dispatch effect, got {}",
						state.batch.effects.len()
					));
				}
				match state.batch.effects.remove(0) {
					NuEffect::Dispatch(invocation) => return Ok(invocation),
					_ => return Err(format!("Nu decode error at {field_path}: expected dispatch effect record")),
				}
			}

			let effect = decode_effect_record(val, &budget, &mut state)?;
			match effect {
				NuEffect::Dispatch(invocation) => Ok(invocation),
				NuEffect::Notify { .. }
				| NuEffect::StopPropagation
				| NuEffect::EditText { .. }
				| NuEffect::SetClipboard { .. }
				| NuEffect::StateSet { .. }
				| NuEffect::StateUnset { .. }
				| NuEffect::ScheduleSet { .. }
				| NuEffect::ScheduleCancel { .. } => Err(format!("Nu decode error at {field_path}: expected dispatch effect record")),
			}
		}
		other => Err(format!("Nu decode error at {field_path}: expected effect record, got {}", other.get_type())),
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeSurface {
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
	decode_lenient_value(value, &budget, &mut state)?;
	Ok(state.batch)
}

/// Lenient decoder for the `xeno effects normalize` command.
///
/// Accepts bare effect records, lists of effect records, nothing, or
/// already-wrapped envelopes. Surface-aware: macro surface still rejects
/// `stop` effects.
pub fn decode_effects_lenient(value: Value, budget: DecodeBudget, surface: DecodeSurface) -> Result<NuEffectBatch, String> {
	let mut state = DecodeState::new(surface);
	decode_lenient_value(value, &budget, &mut state)?;
	Ok(state.batch)
}

fn decode_lenient_value(value: Value, budget: &DecodeBudget, state: &mut DecodeState<'_>) -> Result<(), String> {
	state.visit_node(budget)?;
	match value {
		Value::Nothing { .. } => Ok(()),
		Value::Record { ref val, .. } => {
			if val.contains(EFFECT_FIELD_EFFECTS) {
				// Already an envelope — decode strictly.
				decode_envelope_record(val, budget, state)
			} else {
				// Bare effect record — decode as single effect.
				let effect = decode_effect_record(val, budget, state)?;
				state.push_effect(effect, budget)?;
				Ok(())
			}
		}
		Value::List { vals, .. } => decode_effect_list(vals, budget, state),
		Value::String { .. } => {
			Err(state.err("string returns are not supported; return typed effects via built-ins: xeno effect, xeno effects normalize, xeno call"))
		}
		other => Err(state.err(format_args!("expected effect record, list, or nothing, got {}", other.get_type()))),
	}
}

fn decode_envelope_record(record: &Record, budget: &DecodeBudget, state: &mut DecodeState<'_>) -> Result<(), String> {
	if !record.contains(EFFECT_FIELD_EFFECTS) {
		return Err(state.err("expected envelope record with 'effects' field"));
	}

	state.path.push_field(EFFECT_FIELD_SCHEMA_VERSION);
	let schema_version = match record.get(EFFECT_FIELD_SCHEMA_VERSION) {
		None => EFFECT_SCHEMA_VERSION,
		Some(Value::Int { val, .. }) => *val,
		Some(other) => {
			let err = state.err(format_args!("expected int, got {}", other.get_type()));
			state.path.pop();
			return Err(err);
		}
	};
	state.path.pop();

	if schema_version < 1 {
		return Err(state.err(format_args!("schema_version must be >= 1, got {schema_version}")));
	}
	if schema_version > EFFECT_SCHEMA_VERSION {
		return Err(state.err(format_args!(
			"unsupported schema_version {schema_version} (host supports {EFFECT_SCHEMA_VERSION})"
		)));
	}
	state.batch.schema_version = schema_version;

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

	// Decode warnings if present.
	if record.contains(EFFECT_FIELD_WARNINGS) {
		state.path.push_field(EFFECT_FIELD_WARNINGS);
		if let Some(Value::List { vals, .. }) = record.get(EFFECT_FIELD_WARNINGS) {
			for (idx, val) in vals.iter().enumerate() {
				if state.batch.warnings.len() >= budget.max_effects {
					break;
				}
				state.path.push_index(idx);
				if let Value::String { val, .. } = val
					&& val.len() <= budget.max_string_len
				{
					state.batch.warnings.push(val.clone());
				}
				state.path.pop();
			}
		}
		state.path.pop();
	}

	result
}

fn decode_effect_list(values: Vec<Value>, budget: &DecodeBudget, state: &mut DecodeState<'_>) -> Result<(), String> {
	for (idx, item) in values.into_iter().enumerate() {
		state.path.push_index(idx);
		state.visit_node(budget)?;
		match item {
			Value::Nothing { .. } => {}
			Value::Record { ref val, .. } => {
				if val.contains(EFFECT_FIELD_EFFECTS) {
					// Envelope inside list — flatten.
					decode_envelope_record(val, budget, state)?;
				} else {
					let effect = decode_effect_record(val, budget, state)?;
					state.push_effect(effect, budget)?;
				}
			}
			Value::List { vals, .. } => {
				// Nested list — recurse.
				decode_effect_list(vals, budget, state)?;
			}
			other => {
				let err = state.err(format_args!("expected effect record, envelope, list, or nothing, got {}", other.get_type()));
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
		EFFECT_TYPE_CLIPBOARD => {
			let text = required_string_field_allow_empty(record, EFFECT_FIELD_TEXT, budget, state)?;
			Ok(NuEffect::SetClipboard { text })
		}
		EFFECT_TYPE_STATE => {
			let op = required_string_field(record, EFFECT_FIELD_OP, budget, state)?;
			match op.as_str() {
				"set" => {
					let key = required_string_field(record, EFFECT_FIELD_KEY, budget, state)?;
					let value = required_string_field_allow_empty(record, EFFECT_FIELD_VALUE, budget, state)?;
					Ok(NuEffect::StateSet { key, value })
				}
				"unset" => {
					let key = required_string_field(record, EFFECT_FIELD_KEY, budget, state)?;
					Ok(NuEffect::StateUnset { key })
				}
				other => Err(state.err(format_args!("unknown state op '{other}'; expected 'set' or 'unset'"))),
			}
		}
		EFFECT_TYPE_SCHEDULE => {
			let op = required_string_field(record, EFFECT_FIELD_OP, budget, state)?;
			match op.as_str() {
				"set" => {
					let key = required_string_field(record, EFFECT_FIELD_KEY, budget, state)?;
					let delay_ms = required_u64_field(record, EFFECT_FIELD_DELAY_MS, state)?;
					if delay_ms > MAX_SCHEDULE_DELAY_MS {
						return Err(state.err(format_args!("delay_ms exceeds max {MAX_SCHEDULE_DELAY_MS}")));
					}
					let name = required_string_field(record, EFFECT_FIELD_MACRO, budget, state)?;
					let args = optional_string_list_field(record, EFFECT_FIELD_ARGS, budget, state)?.unwrap_or_default();
					Ok(NuEffect::ScheduleSet { key, delay_ms, name, args })
				}
				"cancel" => {
					let key = required_string_field(record, EFFECT_FIELD_KEY, budget, state)?;
					Ok(NuEffect::ScheduleCancel { key })
				}
				other => Err(state.err(format_args!("unknown schedule op '{other}'; expected 'set' or 'cancel'"))),
			}
		}
		EFFECT_TYPE_EDIT => {
			let op_raw = required_string_field(record, EFFECT_FIELD_OP, budget, state)?;
			let Some(op) = NuTextEditOp::parse(&op_raw) else {
				return Err(state.err(format_args!("unknown edit op '{op_raw}'; expected 'replace_selection' or 'replace_line'")));
			};
			let text = required_string_field_allow_empty(record, EFFECT_FIELD_TEXT, budget, state)?;
			if matches!(op, NuTextEditOp::ReplaceLine) && text.contains(['\n', '\r']) {
				return Err(state.err("replace_line text must not contain newline characters"));
			}
			Ok(NuEffect::EditText { op, text })
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
		schema::KIND_COMMAND => Ok(Invocation::Command(CommandInvocation {
			name,
			args: optional_string_list_field(record, schema::ARGS, budget, state)?.unwrap_or_default(),
			route: CommandRoute::Auto,
		})),
		schema::KIND_EDITOR => Ok(Invocation::Command(CommandInvocation {
			name,
			args: optional_string_list_field(record, schema::ARGS, budget, state)?.unwrap_or_default(),
			route: CommandRoute::Editor,
		})),
		schema::KIND_NU => Ok(Invocation::Nu {
			name,
			args: optional_string_list_field(record, schema::ARGS, budget, state)?.unwrap_or_default(),
		}),
		other => Err(state.err(format_args!("unknown invocation kind '{other}'"))),
	}
}

fn required_string_field_allow_empty<'a>(record: &Record, field: &'a str, budget: &DecodeBudget, state: &mut DecodeState<'a>) -> Result<String, String> {
	state.path.push_field(field);
	let out = match record.get(field) {
		Some(Value::String { val, .. }) => {
			validate_string_limit(state, val, budget)?;
			Ok(val.clone())
		}
		Some(other) => Err(state.err(format_args!("expected string, got {}", other.get_type()))),
		None => Err(state.err("missing required field")),
	};
	state.path.pop();
	out
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

fn required_u64_field<'a>(record: &Record, field: &'a str, state: &mut DecodeState<'a>) -> Result<u64, String> {
	state.path.push_field(field);
	let out = match record.get(field) {
		Some(Value::Int { val, .. }) if *val >= 0 => Ok(*val as u64),
		Some(Value::Int { .. }) => Err(state.err("must be non-negative")),
		Some(other) => Err(state.err(format_args!("expected int, got {}", other.get_type()))),
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
