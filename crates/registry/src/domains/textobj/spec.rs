use serde::{Deserialize, Serialize};

pub use crate::defs::spec::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextObjectsSpec {
	pub text_objects: Vec<TextObjectSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextObjectSpec {
	pub common: MetaCommonSpec,
	pub trigger: String,
	pub alt_triggers: Vec<String>,
}
