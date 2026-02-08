use serde::{Deserialize, Serialize};

pub use crate::defs::spec::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionsSpec {
	pub options: Vec<OptionSpec>,
}

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
