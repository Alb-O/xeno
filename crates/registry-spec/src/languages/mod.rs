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
	pub viewport_repair: Option<ViewportRepairSpec>,
	pub queries: Vec<LanguageQuerySpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportRepairSpec {
	pub enabled: bool,

	/// Scan budget within the window (bytes). Hard cap for O(1) behavior.
	pub max_scan_bytes: u32,

	/// If true, attempt a quick forward search for a real closer before synthesizing.
	pub prefer_real_closer: bool,
	pub max_forward_search_bytes: u32,

	/// Rules used by the scanner.
	pub rules: Vec<ViewportRepairRuleSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViewportRepairRuleSpec {
	/// e.g. /* ... */
	BlockComment {
		open: String,
		close: String,
		nestable: bool,
	},

	/// e.g. "..." or '...'
	String {
		quote: String,
		escape: Option<String>,
	},

	/// e.g. //
	LineComment { start: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagesSpec {
	pub langs: Vec<LanguageSpec>,
}
