use serde::{Deserialize, Serialize};

pub use crate::defs::spec::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuttersSpec {
	pub gutters: Vec<GutterSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GutterSpec {
	pub common: MetaCommonSpec,
	pub width: String,
	pub enabled: bool,
}
