//! Grammar specification schema.
//!
//! Declares grammar bundle metadata and query-file mappings used by language
//! loading and syntax highlighting.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarsSpec {
	#[serde(default)]
	pub grammars: Vec<GrammarSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarSpec {
	pub id: String,
	pub source: GrammarSourceSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GrammarSourceSpec {
	Git {
		remote: String,
		revision: String,
		#[serde(default)]
		subpath: Option<String>,
	},
	Local {
		path: String,
	},
}
