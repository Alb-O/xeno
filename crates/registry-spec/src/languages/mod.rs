//! Language specification schema.
//!
//! Defines language metadata, matching rules, LSP associations, and viewport
//! repair/query configuration used by runtime language loading.

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
	#[serde(default)]
	pub scope: Option<String>,
	#[serde(default)]
	pub grammar_name: Option<String>,
	#[serde(default)]
	pub injection_regex: Option<String>,
	#[serde(default)]
	pub auto_format: bool,
	#[serde(default)]
	pub extensions: Vec<String>,
	#[serde(default)]
	pub filenames: Vec<String>,
	#[serde(default)]
	pub globs: Vec<String>,
	#[serde(default)]
	pub shebangs: Vec<String>,
	#[serde(default)]
	pub comment_tokens: Vec<String>,
	#[serde(default)]
	pub block_comment: Option<(String, String)>,
	#[serde(default)]
	pub lsp_servers: Vec<String>,
	#[serde(default)]
	pub roots: Vec<String>,
	#[serde(default)]
	pub viewport_repair: Option<ViewportRepairSpec>,
	#[serde(default)]
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
	BlockComment { open: String, close: String, nestable: bool },

	/// e.g. "..." or '...'
	String { quote: String, escape: Option<String> },

	/// e.g. //
	LineComment { start: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagesSpec {
	#[serde(default)]
	pub langs: Vec<LanguageSpec>,
}
