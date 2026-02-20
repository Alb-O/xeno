//! Hook specification schema.
//!
//! Describes lifecycle/event hook definitions and execution metadata for
//! runtime registration.

use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpec {
	pub common: MetaCommonSpec,
	pub event: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksSpec {
	#[serde(default)]
	pub hooks: Vec<HookSpec>,
}
