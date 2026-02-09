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
	pub gutters: Vec<GutterSpec>,
}
