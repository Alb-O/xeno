//! Options registry
//!
//! Options are named settings that can be configured globally or per-buffer.
//! This crate provides:
//! - Type definitions ([`OptionDef`], [`OptionValue`], [`OptionType`], [`OptionScope`])
//! - Distributed slice ([`OPTIONS`])
//! - Registration macro ([`option!`])
//! - Standard library implementations (indent, display, behavior, etc.)

use linkme::distributed_slice;

mod impls;
mod macros;

pub use evildoer_registry_motions::{RegistryMetadata, RegistrySource, impl_registry_metadata};

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
	/// Unique identifier (e.g., "evildoer_core::tab_width").
	pub id: &'static str,
	/// Option name (e.g., "tab_width").
	pub name: &'static str,
	/// Short description.
	pub description: &'static str,
	/// The type of value this option accepts.
	pub value_type: OptionType,
	/// Default value (as a callable function).
	pub default: fn() -> OptionValue,
	/// Scope of the option.
	pub scope: OptionScope,
	/// Priority for ordering (lower runs first).
	pub priority: i16,
	/// Origin of the option.
	pub source: RegistrySource,
}

impl core::fmt::Debug for OptionDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("OptionDef")
			.field("name", &self.name)
			.field("value_type", &self.value_type)
			.field("scope", &self.scope)
			.field("priority", &self.priority)
			.field("description", &self.description)
			.finish()
	}
}

/// Registry of all option definitions.
#[distributed_slice]
pub static OPTIONS: [OptionDef];

/// Finds an option definition by name.
pub fn find(name: &str) -> Option<&'static OptionDef> {
	OPTIONS.iter().find(|o| o.name == name)
}

/// Returns all registered options.
pub fn all() -> impl Iterator<Item = &'static OptionDef> {
	OPTIONS.iter()
}

impl_registry_metadata!(OptionDef);
