use crate::helixc::analyzer::types::Type;
use crate::helixc::parser::types::*;

/// Check if a property name is a reserved property and return its expected type
pub(crate) fn get_reserved_property_type(prop_name: &str, item_type: &Type) -> Option<FieldType> {
	match prop_name {
		"id" | "ID" | "Id" => Some(FieldType::Uuid),
		"label" | "Label" => Some(FieldType::String),
		"version" | "Version" => Some(FieldType::I8),
		"from_node" | "fromNode" | "FromNode" => {
			// Only valid for edges
			match item_type {
				Type::Edge(_) | Type::Edges(_) => Some(FieldType::Uuid),
				_ => None,
			}
		}
		"to_node" | "toNode" | "ToNode" => {
			// Only valid for edges
			match item_type {
				Type::Edge(_) | Type::Edges(_) => Some(FieldType::Uuid),
				_ => None,
			}
		}
		"deleted" | "Deleted" => {
			// Only valid for vectors
			match item_type {
				Type::Vector(_) | Type::Vectors(_) => Some(FieldType::Boolean),
				_ => None,
			}
		}
		"level" | "Level" => {
			// Only valid for vectors
			match item_type {
				Type::Vector(_) | Type::Vectors(_) => Some(FieldType::U64),
				_ => None,
			}
		}
		"distance" | "Distance" => {
			// Only valid for vectors
			match item_type {
				Type::Vector(_) | Type::Vectors(_) => Some(FieldType::F64),
				_ => None,
			}
		}
		"data" | "Data" => {
			// Only valid for vectors
			match item_type {
				Type::Vector(_) | Type::Vectors(_) => {
					Some(FieldType::Array(Box::new(FieldType::F64)))
				}
				_ => None,
			}
		}
		_ => None,
	}
}

/// Checks if a traversal is a "simple" property access (no graph navigation steps)
/// and returns the variable name and property name if so.
///
/// A simple traversal is one that only accesses properties on an already-bound variable,
/// without any graph navigation (Out, In, etc.). For example: `toUser::{login}`
///
/// Returns: Some((variable_name, property_name)) if simple, None otherwise
pub(crate) fn is_simple_property_traversal(tr: &Traversal) -> Option<(String, String)> {
	// Check if the start is an identifier (not a type-based query)
	let var_name = match &tr.start {
		StartNode::Identifier(id) => id.clone(),
		_ => return None,
	};

	// Check if there's exactly one step and it's an Object (property access)
	if tr.steps.len() != 1 {
		return None;
	}

	// Check if the single step is an Object step (property access like {login})
	match &tr.steps[0].step {
		StepType::Object(obj) => {
			// Check if it's a simple property fetch (single field, no spread)
			if obj.fields.len() == 1 && !obj.should_spread {
				let field = &obj.fields[0];
				// Check if it's a simple field selection (Empty or Identifier, not a complex expression)
				match &field.value.value {
					FieldValueType::Empty | FieldValueType::Identifier(_) => {
						return Some((var_name, field.key.clone()));
					}
					_ => return None,
				}
			}
			None
		}
		_ => None,
	}
}
