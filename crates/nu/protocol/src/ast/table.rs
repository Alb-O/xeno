use serde::{Deserialize, Serialize};

use super::Expression;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
	pub columns: Box<[Expression]>,
	pub rows: Box<[Box<[Expression]>]>,
}
