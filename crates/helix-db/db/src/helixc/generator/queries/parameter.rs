use std::fmt::{self, Display};

use crate::helixc::generator::utils::GeneratedType;

pub struct Parameter {
	pub name: String,
	pub field_type: GeneratedType,
	pub is_optional: bool,
}

impl Display for Parameter {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.is_optional {
			true => write!(f, "pub {}: Option<{}>", self.name, self.field_type),
			false => write!(f, "pub {}: {}", self.name, self.field_type),
		}
	}
}
