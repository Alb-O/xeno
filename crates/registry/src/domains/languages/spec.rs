use serde::{Deserialize, Serialize};

pub use crate::defs::spec::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagesSpec {
	pub langs: Vec<LanguageSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageSpec {
	pub common: MetaCommonSpec,
	pub scope: Option<String>,
	pub grammar_name: Option<String>,
	pub injection_regex: Option<String>,
	pub auto_format: bool,
	pub extensions: Vec<String>,
	pub filenames: Vec<String>,
	pub globs: Vec<String>,
	pub shebangs: Vec<String>,
	pub comment_tokens: Vec<String>,
	pub block_comment: Option<(String, String)>,
	pub lsp_servers: Vec<String>,
	pub roots: Vec<String>,
}
