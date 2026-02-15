//! LSP server specification schema.
//!
//! Declares server process configuration and language attachment metadata.

#[cfg(feature = "compile")]
pub mod compile;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServersSpec {
	#[serde(default)]
	pub servers: Vec<LspServerSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerSpec {
	pub common: MetaCommonSpec,
	pub command: String,
	#[serde(default)]
	pub args: Vec<String>,
	#[serde(default)]
	pub environment: BTreeMap<String, String>,
	#[serde(default)]
	pub config_json: Option<String>,
	#[serde(default)]
	pub source: Option<String>,
	#[serde(default)]
	pub nix: Option<String>,
}
