use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
	pub common: MetaCommonSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandsSpec {
	pub commands: Vec<CommandSpec>,
}
