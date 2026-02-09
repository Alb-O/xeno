use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServersSpec {
	pub servers: Vec<LspServerSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerSpec {
	pub common: MetaCommonSpec,
	pub command: String,
	pub args: Vec<String>,
	pub environment: BTreeMap<String, String>,
	pub config_json: Option<String>,
	pub source: Option<String>,
	pub nix: Option<String>,
}
