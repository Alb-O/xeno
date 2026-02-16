use serde::{Deserialize, Serialize};

use super::Expression;
use crate::{Spanned, Unit};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueWithUnit {
	pub expr: Expression,
	pub unit: Spanned<Unit>,
}
