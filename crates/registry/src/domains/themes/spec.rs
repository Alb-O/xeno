use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::defs::spec::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemesSpec {
	pub themes: Vec<ThemeSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSpec {
	pub common: MetaCommonSpec,
	pub variant: String,
	pub palette: HashMap<String, String>,
	pub ui: HashMap<String, String>,
	pub mode: HashMap<String, String>,
	pub semantic: HashMap<String, String>,
	pub popup: HashMap<String, String>,
	pub syntax: HashMap<String, RawStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawStyle {
	pub fg: Option<String>,
	pub bg: Option<String>,
	pub modifiers: Option<String>,
}
