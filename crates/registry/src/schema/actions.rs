//! Action specification schema.
//!
//! Defines action metadata and keybinding declarations used to build runtime
//! action registries.

use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

pub const VALID_MODES: &[&str] = &["normal", "insert", "match", "space"];
pub const VALID_CAPS: &[&str] = &[
	"Text",
	"Cursor",
	"Selection",
	"Mode",
	"Messaging",
	"Edit",
	"Search",
	"Undo",
	"FileOps",
	"Overlay",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSpec {
	pub common: MetaCommonSpec,
	#[serde(default)]
	pub bindings: Vec<KeyBindingSpec>,
	#[serde(default)]
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
	#[serde(default)]
	pub actions: Vec<ActionSpec>,
	#[serde(default)]
	pub prefixes: Vec<KeyPrefixSpec>,
}
