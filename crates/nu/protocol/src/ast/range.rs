use serde::{Deserialize, Serialize};

use super::{Expression, RangeOperator};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Range {
	pub from: Option<Expression>,
	pub next: Option<Expression>,
	pub to: Option<Expression>,
	pub operator: RangeOperator,
}
