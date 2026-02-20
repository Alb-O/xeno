//! Gutter annotation specification schema.
//!
//! Defines declarative gutter kinds and visual attributes for registry loading.


use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

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
