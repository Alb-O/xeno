//! Options/settings system type definitions.
//!
//! Options are named settings that can be configured globally or per-buffer.
//! They are registered at compile-time using `linkme`.

use linkme::distributed_slice;
use crate::RegistrySource;

/// Registry of all option definitions.
#[distributed_slice]
pub static OPTIONS: [OptionDef];

/// The value of an option.
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
	Bool(bool),
	Int(i64),
	String(String),
}

impl OptionValue {
	pub fn as_bool(&self) -> Option<bool> {
		match self {
			OptionValue::Bool(v) => Some(*v),
			_ => None,
		}
	}

	pub fn as_int(&self) -> Option<i64> {
		match self {
			OptionValue::Int(v) => Some(*v),
			_ => None,
		}
	}

	pub fn as_str(&self) -> Option<&str> {
		match self {
			OptionValue::String(v) => Some(v),
			_ => None,
		}
	}
}

impl From<bool> for OptionValue {
	fn from(v: bool) -> Self {
		OptionValue::Bool(v)
	}
}

impl From<i64> for OptionValue {
	fn from(v: i64) -> Self {
		OptionValue::Int(v)
	}
}

impl From<String> for OptionValue {
	fn from(v: String) -> Self {
		OptionValue::String(v)
	}
}

impl From<&str> for OptionValue {
	fn from(v: &str) -> Self {
		OptionValue::String(v.to_string())
	}
}

/// The type of an option's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
	Bool,
	Int,
	String,
}

/// Scope for option application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionScope {
	/// Global option (applies to all buffers).
	Global,
	/// Buffer-local option (can be overridden per-buffer).
	Buffer,
}

/// Definition of a configurable option.
pub struct OptionDef {
	/// Unique identifier.
	pub id: &'static str,
	/// Option name (e.g., "indent_width", "tab_stop").
	pub name: &'static str,
	/// Short description.
	pub description: &'static str,
	/// The type of value this option accepts.
	pub value_type: OptionType,
	/// Default value.
	pub default: fn() -> OptionValue,
	/// Scope of the option.
	pub scope: OptionScope,
	/// Origin of the option.
	pub source: RegistrySource,
}

impl std::fmt::Debug for OptionDef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("OptionDef")
			.field("name", &self.name)
			.field("value_type", &self.value_type)
			.field("scope", &self.scope)
			.field("description", &self.description)
			.finish()
	}
}

/// Find an option definition by name.
pub fn find_option(name: &str) -> Option<&'static OptionDef> {
	OPTIONS.iter().find(|o| o.name == name)
}

/// Get all registered options.
pub fn all_options() -> &'static [OptionDef] {
	&OPTIONS
}
