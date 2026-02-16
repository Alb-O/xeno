//! Gutter annotation specification schema.
//!
//! Defines declarative gutter kinds and visual attributes for registry loading.

#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GutterSpec {
	pub common: MetaCommonSpec,
	pub width: String,
	pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuttersSpec {
	#[serde(default)]
	pub gutters: Vec<GutterSpec>,
}
