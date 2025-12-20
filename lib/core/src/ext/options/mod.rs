//! Options/settings system for editor configuration.
//!
//! Options are named settings that can be configured globally or per-buffer.
//! They are registered at compile-time using `linkme` and can be queried
//! and modified at runtime.
//!
//! Each file in this directory defines options for a specific category.

mod behavior;
mod display;
mod file;
mod indent;
mod scroll;
mod search;

use linkme::distributed_slice;

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
	pub source: crate::ext::ExtensionSource,
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_option_value_conversions() {
		let b: OptionValue = true.into();
		assert_eq!(b.as_bool(), Some(true));
		assert_eq!(b.as_int(), None);

		let i: OptionValue = 42i64.into();
		assert_eq!(i.as_int(), Some(42));
		assert_eq!(i.as_bool(), None);

		let s: OptionValue = "hello".into();
		assert_eq!(s.as_str(), Some("hello"));
		assert_eq!(s.as_bool(), None);
	}

	#[test]
	fn test_default_options_registered() {
		assert!(find_option("tab_width").is_some());
		assert!(find_option("indent_width").is_some());
		assert!(find_option("use_tabs").is_some());
		assert!(find_option("line_numbers").is_some());
		assert!(find_option("wrap_lines").is_some());
		assert!(find_option("scroll_margin").is_some());
	}

	#[test]
	fn test_option_defaults() {
		let tab_width = find_option("tab_width").unwrap();
		assert_eq!((tab_width.default)().as_int(), Some(4));

		let use_tabs = find_option("use_tabs").unwrap();
		assert_eq!((use_tabs.default)().as_bool(), Some(false));
	}

	#[test]
	fn test_all_options() {
		let opts = all_options();
		assert!(opts.len() >= 6);
	}

	#[test]
	fn test_new_options_registered() {
		assert!(find_option("cursorline").is_some());
		assert!(find_option("search_smart_case").is_some());
		assert!(find_option("mouse").is_some());
		assert!(find_option("final_newline").is_some());
	}
}
