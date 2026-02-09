use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpec {
	pub common: MetaCommonSpec,
	pub event: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksSpec {
	pub hooks: Vec<HookSpec>,
}
