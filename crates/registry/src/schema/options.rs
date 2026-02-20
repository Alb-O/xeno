//! Option specification schema.
//!
//! Defines configurable option metadata, typing, defaults, and scope.


use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

pub const VALID_TYPES: &[&str] = &["bool", "int", "string"];
pub const VALID_SCOPES: &[&str] = &["buffer", "global"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionSpec {
	/// Common metadata.
	pub common: MetaCommonSpec,
	/// Config key (e.g., `"tab-width"`).
	pub key: String,
	/// Value type: `"bool"`, `"int"`, `"string"`.
	pub value_type: String,
	/// Default value as a string.
	pub default: String,
	/// Scope: `"buffer"` or `"global"`.
	pub scope: String,
	/// Optional validator name.
	#[serde(default)]
	pub validator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsSpec {
	#[serde(default)]
	pub options: Vec<OptionSpec>,
}
