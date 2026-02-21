use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaCommonSpec {
	pub name: String,
	#[serde(default)]
	pub description: String,
	#[serde(default)]
	pub short_desc: Option<String>,
	#[serde(default)]
	pub keys: Vec<String>,
	#[serde(default)]
	pub priority: i16,
	#[serde(default)]
	pub mutates_buffer: bool,
}
