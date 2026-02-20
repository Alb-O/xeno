//! Snippet specification schema.
//!
//! Declares snippet definitions and associated metadata for runtime expansion.

use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

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
