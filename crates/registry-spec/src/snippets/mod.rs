#[cfg(feature = "compile")]
pub mod compile;

use serde::{Deserialize, Serialize};

use crate::MetaCommonSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetSpec {
	pub common: MetaCommonSpec,
	pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetsSpec {
	#[serde(default)]
	pub snippets: Vec<SnippetSpec>,
}
