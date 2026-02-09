#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageQuerySpec {
	pub kind: String, // e.g. "highlights"
	pub text: String, // full .scm contents
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
	pub queries: Vec<LanguageQuerySpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagesSpec {
	pub langs: Vec<LanguageSpec>,
}
