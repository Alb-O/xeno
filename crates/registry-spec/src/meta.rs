use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaCommonSpec {
	pub name: String,
	pub description: String,
	pub short_desc: Option<String>,
	pub keys: Vec<String>,
	pub priority: i16,
	pub caps: Vec<String>,
	pub flags: u32,
}
