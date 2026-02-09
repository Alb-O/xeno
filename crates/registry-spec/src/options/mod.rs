#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

pub const VALID_TYPES: &[&str] = &["bool", "int", "string"];
pub const VALID_SCOPES: &[&str] = &["buffer", "global"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionSpec {
	/// Common metadata.
	pub common: MetaCommonSpec,
	/// KDL config key (e.g., `"tab-width"`).
	pub kdl_key: String,
	/// Value type: `"bool"`, `"int"`, `"string"`.
	pub value_type: String,
	/// Default value as a string.
	pub default: String,
	/// Scope: `"buffer"` or `"global"`.
	pub scope: String,
	/// Optional validator name.
	pub validator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsSpec {
	pub options: Vec<OptionSpec>,
}
