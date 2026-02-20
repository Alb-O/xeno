//! Text-object specification schema.
//!
//! Defines text object metadata used by selection and motion systems.

use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextObjectSpec {
	pub common: MetaCommonSpec,
	pub trigger: String,
	#[serde(default)]
	pub alt_triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextObjectsSpec {
	#[serde(default)]
	pub text_objects: Vec<TextObjectSpec>,
}
