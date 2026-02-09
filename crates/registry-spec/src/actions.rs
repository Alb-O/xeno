use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSpec {
	pub common: MetaCommonSpec,
	pub bindings: Vec<KeyBindingSpec>,
	pub group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindingSpec {
	pub mode: String,
	pub keys: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPrefixSpec {
	pub mode: String,
	pub keys: String,
	pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionsSpec {
	pub actions: Vec<ActionSpec>,
	pub prefixes: Vec<KeyPrefixSpec>,
}
