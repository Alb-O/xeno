use serde::{Deserialize, Serialize};

use super::Expression;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
	pub expr: Expression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributeBlock {
	pub attributes: Vec<Attribute>,
	pub item: Box<Expression>,
}
