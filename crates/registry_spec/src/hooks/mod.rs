//! Hook specification schema.
//!
//! Describes lifecycle/event hook definitions and execution metadata for
//! runtime registration.

#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

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
