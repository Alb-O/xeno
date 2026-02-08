use serde::{Deserialize, Serialize};

/// Raw option metadata extracted from KDL.
#[derive(Debug, Serialize, Deserialize)]
pub struct OptionMetaRaw {
	/// Common metadata.
	pub common: super::common::MetaCommonRaw,
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

/// Top-level blob containing all option metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct OptionsBlob {
	/// All option definitions.
	pub options: Vec<OptionMetaRaw>,
}
