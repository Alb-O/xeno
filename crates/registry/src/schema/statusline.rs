//! Statusline specification schema.
//!
//! Defines statusline segment templates and placement metadata.

use serde::{Deserialize, Serialize};

use super::meta::MetaCommonSpec;

pub const VALID_POSITIONS: &[&str] = &["left", "right", "center"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineSegmentSpec {
	pub common: MetaCommonSpec,
	pub position: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineSpec {
	#[serde(default)]
	pub segments: Vec<StatuslineSegmentSpec>,
}
