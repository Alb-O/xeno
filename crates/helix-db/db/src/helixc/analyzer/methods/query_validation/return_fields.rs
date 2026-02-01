use super::utils::capitalize_first;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::types::Type;
use crate::helixc::generator::return_values::{
	ReturnFieldInfo, ReturnFieldSource, ReturnFieldType, RustFieldType,
};
use crate::helixc::generator::traversal_steps::{ShouldCollect, Traversal as GeneratedTraversal};
use crate::helixc::generator::utils::{GenRef, RustType};

/// Build unified field list for return types
/// This handles all cases: simple schema, projections, spread, nested traversals
pub(crate) fn build_return_fields(
	ctx: &Ctx,
	inferred_type: &Type,
	traversal: &GeneratedTraversal,
	struct_name_prefix: &str,
) -> Vec<ReturnFieldInfo> {
	let mut fields = Vec::new();

	// Handle aggregate types specially
	if let Type::Aggregate(info) = inferred_type {
		// All aggregates have a key field (the grouping key from HashMap)
		fields.push(ReturnFieldInfo::new_implicit(
			"key".to_string(),
			RustFieldType::Primitive(GenRef::Std(RustType::String)),
		));

		// Add fields for each grouped property
		// We need to get the source type's schema to determine property types
		let (_schema_fields, _item_type) = match info.source_type.as_ref() {
			Type::Node(Some(label)) | Type::Nodes(Some(label)) => {
				(ctx.node_fields.get(label.as_str()), "node")
			}
			Type::Edge(Some(label)) | Type::Edges(Some(label)) => {
				(ctx.edge_fields.get(label.as_str()), "edge")
			}
			Type::Vector(Some(label)) | Type::Vectors(Some(label)) => {
				(ctx.vector_fields.get(label.as_str()), "vector")
			}
			_ => (None, "unknown"),
		};

		// Add each grouped property as a field
		for prop_name in &info.properties {
			fields.push(ReturnFieldInfo::new_schema(
				prop_name.clone(),
				RustFieldType::OptionValue,
			));
		}

		// Add count field
		fields.push(ReturnFieldInfo::new_implicit(
			"count".to_string(),
			RustFieldType::Primitive(GenRef::Std(RustType::I32)),
		));

		// For non-COUNT aggregates, add items field with nested struct
		if !info.is_count {
			// Build nested struct for the items
			let items_struct_name = format!("{}Items", struct_name_prefix);
			// Recursively build fields for the source type
			let item_fields = build_return_fields(
				ctx,
				info.source_type.as_ref(),
				traversal,
				&items_struct_name,
			);

			// Create field with proper nested_struct_name to avoid conflicts
			fields.push(ReturnFieldInfo {
				name: "items".to_string(),
				field_type: ReturnFieldType::Nested(item_fields),
				source: ReturnFieldSource::NestedTraversal {
					traversal_expr: String::new(),
					traversal_code: None,
					nested_struct_name: Some(format!("{}ReturnType", items_struct_name)),
					traversal_type: None,
					closure_param_name: None,
					closure_source_var: None,
					accessed_field_name: None,
					own_closure_param: None,
					requires_full_traversal: false,
					is_first: false,
				},
			});
		}

		return fields;
	}

	// Get schema type name if this is a schema type
	let schema_type = match inferred_type {
		Type::Node(Some(label)) | Type::Nodes(Some(label)) => Some((label.as_str(), "node")),
		Type::Edge(Some(label)) | Type::Edges(Some(label)) => Some((label.as_str(), "edge")),
		Type::Vector(Some(label)) | Type::Vectors(Some(label)) => Some((label.as_str(), "vector")),
		_ => None,
	};

	// Step 1: Add implicit fields if this is a schema type
	if let Some((label, item_type)) = schema_type {
		// Helper to find which output field name maps to a given property
		// e.g., for property "id", might return Some("file_id") if there's a mapping file_id -> ID
		let find_output_for_property = |property: &str| -> Option<String> {
			// First check if any object_field maps to this property via field_name_mappings
			for output_name in &traversal.object_fields {
				if let Some(prop) = traversal.field_name_mappings.get(output_name)
					&& prop.to_lowercase() == property.to_lowercase()
				{
					return Some(output_name.clone());
				}
				// Also check if the output_name itself matches (identity mapping)
				if output_name.to_lowercase() == property.to_lowercase() {
					return Some(output_name.clone());
				}
			}
			None
		};

		// If has_object_step, only add implicit fields if they're explicitly selected OR has_spread
		// Otherwise, add all implicit fields (default behavior)
		let should_add_field = |field_name: &str| {
			// Exclude if field is in excluded_fields
			if traversal.excluded_fields.contains(&field_name.to_string()) {
				return false;
			}
			// If has object step, only include if explicitly selected (possibly with remapping) OR has_spread
			if traversal.has_object_step {
				find_output_for_property(field_name).is_some() || traversal.has_spread
			} else {
				true
			}
		};

		// Add id and label if no object step OR if explicitly selected
		if should_add_field("id") {
			// Check if id is remapped to a different output name
			if let Some(output_name) = find_output_for_property("id") {
				if output_name != "id" {
					// Remapped: e.g., file_id: ID
					fields.push(ReturnFieldInfo::new_implicit_with_property(
						output_name,
						"id".to_string(),
						RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
					));
				} else {
					fields.push(ReturnFieldInfo::new_implicit(
						"id".to_string(),
						RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
					));
				}
			} else if !traversal.has_object_step || traversal.has_spread {
				// No object step or has spread means return all fields
				fields.push(ReturnFieldInfo::new_implicit(
					"id".to_string(),
					RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
				));
			}
		}
		if should_add_field("label") {
			if let Some(output_name) = find_output_for_property("label") {
				if output_name != "label" {
					fields.push(ReturnFieldInfo::new_implicit_with_property(
						output_name,
						"label".to_string(),
						RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
					));
				} else {
					fields.push(ReturnFieldInfo::new_implicit(
						"label".to_string(),
						RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
					));
				}
			} else if !traversal.has_object_step || traversal.has_spread {
				fields.push(ReturnFieldInfo::new_implicit(
					"label".to_string(),
					RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
				));
			}
		}

		// Add type-specific implicit fields
		if item_type == "edge" {
			if should_add_field("from_node") {
				if let Some(output_name) = find_output_for_property("from_node") {
					if output_name != "from_node" {
						fields.push(ReturnFieldInfo::new_implicit_with_property(
							output_name,
							"from_node".to_string(),
							RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
						));
					} else {
						fields.push(ReturnFieldInfo::new_implicit(
							"from_node".to_string(),
							RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
						));
					}
				} else if !traversal.has_object_step || traversal.has_spread {
					fields.push(ReturnFieldInfo::new_implicit(
						"from_node".to_string(),
						RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
					));
				}
			}
			if should_add_field("to_node") {
				if let Some(output_name) = find_output_for_property("to_node") {
					if output_name != "to_node" {
						fields.push(ReturnFieldInfo::new_implicit_with_property(
							output_name,
							"to_node".to_string(),
							RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
						));
					} else {
						fields.push(ReturnFieldInfo::new_implicit(
							"to_node".to_string(),
							RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
						));
					}
				} else if !traversal.has_object_step || traversal.has_spread {
					fields.push(ReturnFieldInfo::new_implicit(
						"to_node".to_string(),
						RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
					));
				}
			}
		} else if item_type == "vector" {
			if should_add_field("data") {
				if let Some(output_name) = find_output_for_property("data") {
					if output_name != "data" {
						fields.push(ReturnFieldInfo::new_implicit_with_property(
							output_name,
							"data".to_string(),
							RustFieldType::RefArray(RustType::F64),
						));
					} else {
						fields.push(ReturnFieldInfo::new_implicit(
							"data".to_string(),
							RustFieldType::RefArray(RustType::F64),
						));
					}
				} else if !traversal.has_object_step || traversal.has_spread {
					fields.push(ReturnFieldInfo::new_implicit(
						"data".to_string(),
						RustFieldType::RefArray(RustType::F64),
					));
				}
			}
			if should_add_field("score") {
				if let Some(output_name) = find_output_for_property("score") {
					if output_name != "score" {
						fields.push(ReturnFieldInfo::new_implicit_with_property(
							output_name,
							"score".to_string(),
							RustFieldType::Primitive(GenRef::Std(RustType::F64)),
						));
					} else {
						fields.push(ReturnFieldInfo::new_implicit(
							"score".to_string(),
							RustFieldType::Primitive(GenRef::Std(RustType::F64)),
						));
					}
				} else if !traversal.has_object_step || traversal.has_spread {
					fields.push(ReturnFieldInfo::new_implicit(
						"score".to_string(),
						RustFieldType::Primitive(GenRef::Std(RustType::F64)),
					));
				}
			}
		}

		// Step 2: Add schema fields based on projection mode
		let schema_fields = match item_type {
			"node" => ctx.node_fields.get(label),
			"edge" => ctx.edge_fields.get(label),
			"vector" => ctx.vector_fields.get(label),
			_ => None,
		};

		// Helper to check if a property is an implicit field
		let is_implicit_field = |prop: &str| -> bool {
			let lower = prop.to_lowercase();
			matches!(
				lower.as_str(),
				"id" | "label" | "from_node" | "to_node" | "data" | "score"
			)
		};

		if let Some(schema_fields) = schema_fields {
			if traversal.has_object_step {
				// Projection mode - only include selected fields
				for field_name in &traversal.object_fields {
					// Skip if it's a nested traversal (handled separately)
					if traversal.nested_traversals.contains_key(field_name) {
						continue;
					}

					// Skip if it's a computed expression (handled separately)
					if traversal.computed_expressions.contains_key(field_name) {
						continue;
					}

					// Look up the actual property name from the mapping
					let property_name = traversal
						.field_name_mappings
						.get(field_name)
						.unwrap_or(field_name);

					// Skip implicit fields (already handled above)
					if is_implicit_field(property_name) {
						continue;
					}

					if let Some(_field) = schema_fields.get(property_name.as_str()) {
						// If property_name != field_name, we need to track the mapping
						if property_name != field_name {
							fields.push(ReturnFieldInfo::new_schema_with_property(
								field_name.clone(),    // output field name ("post")
								property_name.clone(), // source property name ("content")
								RustFieldType::OptionValue,
							));
						} else {
							fields.push(ReturnFieldInfo::new_schema(
								field_name.clone(),
								RustFieldType::OptionValue,
							));
						}
					}
				}

				// If has_spread, add all remaining schema fields
				if traversal.has_spread {
					for (field_name, _field) in schema_fields.iter() {
						// Skip if output name already exists
						let already_exists = fields.iter().any(|f| f.name == *field_name);
						// Skip if this source property was remapped to a different output name
						let already_remapped = traversal
							.field_name_mappings
							.values()
							.any(|source_prop| source_prop == field_name);
						let already_covered_by_nested =
							traversal.nested_traversals.values().any(|info| {
								info.traversal
									.object_fields
									.iter()
									.any(|f| f.to_lowercase() == field_name.to_lowercase())
							});
						if already_exists || already_remapped || already_covered_by_nested {
							continue;
						}
						// Skip if excluded
						if traversal.excluded_fields.contains(&field_name.to_string()) {
							continue;
						}

						// Check if this is an implicit field - if so, use the correct type
						let is_implicit_field = matches!(
							*field_name,
							"id" | "label" | "from_node" | "to_node" | "data" | "score"
						);

						if is_implicit_field {
							let rust_type = match *field_name {
								"data" => RustFieldType::RefArray(RustType::F64),
								"score" => RustFieldType::Primitive(GenRef::Std(RustType::F64)),
								_ => RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str)),
							};
							fields.push(ReturnFieldInfo::new_implicit(
								field_name.to_string(),
								rust_type,
							));
						} else {
							fields.push(ReturnFieldInfo::new_schema(
								field_name.to_string(),
								RustFieldType::OptionValue,
							));
						}
					}
				}
			} else {
				// No projection - include all schema fields except excluded ones
				for (field_name, _field) in schema_fields.iter() {
					// Skip implicit fields (already added)
					if *field_name == "id"
						|| *field_name == "label"
						|| *field_name == "from_node"
						|| *field_name == "to_node"
						|| *field_name == "data"
						|| *field_name == "score"
					{
						continue;
					}
					// Skip if excluded
					if traversal.excluded_fields.contains(&field_name.to_string()) {
						continue;
					}
					fields.push(ReturnFieldInfo::new_schema(
						field_name.to_string(),
						RustFieldType::OptionValue,
					));
				}
			}
		}
	}

	// Step 3: Add nested traversals
	for (field_name, nested_info) in &traversal.nested_traversals {
		// For nested traversals, extract the return type and build nested fields
		if let Some(ref return_type) = nested_info.return_type {
			// Check if this is a scalar type or needs a struct
			match return_type {
				Type::Count => {
					// Check if this is a variable reference (e.g., `count: count_var`)
					// Variable references have closure_source_var set but no graph steps and no object step
					let is_variable_reference = nested_info.closure_source_var.is_some()
						&& !nested_info.traversal.has_graph_steps()
						&& !nested_info.traversal.has_object_step;

					let trav_code = if is_variable_reference {
						String::new()
					} else {
						nested_info.traversal.format_steps_only()
					};
					let accessed_field_name = nested_info.traversal.object_fields.first().cloned();
					fields.push(ReturnFieldInfo {
						name: field_name.clone(),
						field_type: ReturnFieldType::Simple(RustFieldType::Value),
						source: ReturnFieldSource::NestedTraversal {
							traversal_expr: format!("nested_traversal_{}", field_name),
							traversal_code: Some(trav_code),
							nested_struct_name: None,
							traversal_type: Some(nested_info.traversal.traversal_type.clone()),
							closure_param_name: nested_info.closure_param_name.clone(),
							closure_source_var: nested_info.closure_source_var.clone(),
							accessed_field_name,
							own_closure_param: nested_info.own_closure_param.clone(),
							requires_full_traversal: nested_info.traversal.has_graph_steps(),
							is_first: false,
						},
					});
				}
				Type::Scalar(_scalar_ty) => {
					// Check if the traversal is accessing an implicit field
					// For nested traversals like usr::ID, we need to check what field is actually accessed
					let accessed_field = nested_info.traversal.object_fields.first(); // Get the first (and usually only) field being accessed
					let is_implicit = accessed_field
						.map(|f| {
							matches!(
								f.as_str(),
								"id" | "label"
									| "from_node" | "to_node" | "data"
									| "score" | "ID" | "Label" // Also check capitalized versions
							)
						})
						.unwrap_or(!nested_info.traversal.has_object_step);

					// Check if this has graph steps AND object step - if so, generate nested struct like Node/Edge case
					if nested_info.traversal.has_graph_steps()
						&& nested_info.traversal.has_object_step
					{
						// Generate nested struct for single-field object access with graph navigation
						let nested_prefix =
							format!("{}{}", struct_name_prefix, capitalize_first(field_name));

						// Build the nested fields from the object_fields
						let mut nested_field_infos = Vec::new();
						for obj_field in &nested_info.traversal.object_fields {
							// Check if it's an implicit field
							let field_type = if matches!(
								obj_field.as_str(),
								"id" | "ID" | "label" | "Label" | "from_node" | "to_node"
							) {
								RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str))
							} else if obj_field == "data" {
								RustFieldType::RefArray(RustType::F64)
							} else if obj_field == "score" {
								RustFieldType::Primitive(GenRef::Std(RustType::F64))
							} else {
								RustFieldType::OptionValue
							};

							// Determine if this is an implicit or schema field
							let source = if matches!(
								obj_field.as_str(),
								"id" | "ID"
									| "label" | "Label" | "from_node"
									| "to_node" | "data" | "score"
							) {
								ReturnFieldSource::ImplicitField {
									property_name: None,
								}
							} else {
								ReturnFieldSource::SchemaField {
									property_name: None,
								}
							};

							nested_field_infos.push(ReturnFieldInfo {
								name: obj_field.clone(),
								field_type: ReturnFieldType::Simple(field_type),
								source,
							});
						}

						let nested_struct_name = format!("{}ReturnType", nested_prefix);
						let is_first =
							matches!(nested_info.traversal.should_collect, ShouldCollect::ToObj);

						fields.push(ReturnFieldInfo {
							name: field_name.clone(),
							field_type: ReturnFieldType::Nested(nested_field_infos),
							source: ReturnFieldSource::NestedTraversal {
								traversal_expr: format!("nested_traversal_{}", field_name),
								// Use format_steps_without_property_fetch for scalar types so the
								// property access is handled in the struct mapping, not as a traversal step
								traversal_code: Some(
									nested_info.traversal.format_steps_without_property_fetch(),
								),
								nested_struct_name: Some(nested_struct_name),
								traversal_type: Some(nested_info.traversal.traversal_type.clone()),
								closure_param_name: nested_info.closure_param_name.clone(),
								closure_source_var: nested_info.closure_source_var.clone(),
								accessed_field_name: None,
								own_closure_param: nested_info.own_closure_param.clone(),
								requires_full_traversal: true,
								is_first,
							},
						});
					} else {
						// Simple property access - no graph steps OR no object step
						// If this traversal has graph steps, check if ::FIRST was used
						let rust_type = if nested_info.traversal.has_graph_steps() {
							// Check if ::FIRST was used (should_collect is ToObj)
							if matches!(nested_info.traversal.should_collect, ShouldCollect::ToObj)
							{
								RustFieldType::OptionValue // ::FIRST returns Option<&'a Value>
							} else {
								RustFieldType::Vec(Box::new(RustFieldType::Value))
							}
						} else if is_implicit {
							// Use the appropriate type based on the implicit field
							match accessed_field.map(|s| s.as_str()) {
								Some("data") => RustFieldType::RefArray(RustType::F64),
								Some("score") => {
									RustFieldType::Primitive(GenRef::Std(RustType::F64))
								}
								Some("id") | Some("ID") | Some("label") | Some("Label")
								| Some("from_node") | Some("to_node") | None => {
									RustFieldType::Primitive(GenRef::RefLT("a", RustType::Str))
								}
								_ => RustFieldType::OptionValue,
							}
						} else {
							RustFieldType::OptionValue
						};

						// Check if this is a variable reference (e.g., `scalar: scalar_var`)
						// Variable references have closure_source_var set but no graph steps and no object step
						let is_variable_reference = nested_info.closure_source_var.is_some()
							&& !nested_info.traversal.has_graph_steps()
							&& !nested_info.traversal.has_object_step;

						let trav_code = if is_variable_reference {
							String::new()
						} else {
							nested_info.traversal.format_steps_only()
						};
						// Extract the accessed field name from object_fields
						let accessed_field_name =
							nested_info.traversal.object_fields.first().cloned();
						fields.push(ReturnFieldInfo {
							name: field_name.clone(),
							field_type: ReturnFieldType::Simple(rust_type),
							source: ReturnFieldSource::NestedTraversal {
								traversal_expr: format!("nested_traversal_{}", field_name),
								traversal_code: Some(trav_code),
								nested_struct_name: None,
								traversal_type: Some(nested_info.traversal.traversal_type.clone()),
								closure_param_name: nested_info.closure_param_name.clone(),
								closure_source_var: nested_info.closure_source_var.clone(),
								accessed_field_name,
								own_closure_param: nested_info.own_closure_param.clone(),
								requires_full_traversal: nested_info.traversal.has_graph_steps(),
								is_first: false,
							},
						});
					}
				}
				Type::Node(_)
				| Type::Edge(_)
				| Type::Vector(_)
				| Type::Nodes(_)
				| Type::Edges(_)
				| Type::Vectors(_) => {
					// Check if this is a variable reference (e.g., `user: u` in a closure)
					// Variable references have closure_source_var set but no graph steps and no object step
					let is_variable_reference = nested_info.closure_source_var.is_some()
						&& !nested_info.traversal.has_graph_steps()
						&& !nested_info.traversal.has_object_step;

					// Check if there's property access (object step) - if not, just return TraversalValue
					if !nested_info.traversal.has_object_step && !is_variable_reference {
						// No property access - return simple TraversalValue type
						let rust_type = match return_type {
							Type::Nodes(_) | Type::Edges(_) | Type::Vectors(_) => {
								RustFieldType::Vec(Box::new(RustFieldType::TraversalValue))
							}
							_ => RustFieldType::TraversalValue,
						};

						fields.push(ReturnFieldInfo {
							name: field_name.clone(),
							field_type: ReturnFieldType::Simple(rust_type),
							source: ReturnFieldSource::NestedTraversal {
								traversal_expr: format!("nested_traversal_{}", field_name),
								traversal_code: Some(nested_info.traversal.format_steps_only()),
								nested_struct_name: None,
								traversal_type: Some(nested_info.traversal.traversal_type.clone()),
								closure_param_name: nested_info.closure_param_name.clone(),
								closure_source_var: nested_info.closure_source_var.clone(),
								accessed_field_name: None,
								own_closure_param: nested_info.own_closure_param.clone(),
								requires_full_traversal: nested_info.traversal.has_graph_steps(),
								is_first: false,
							},
						});
					} else {
						// Has property access or variable reference - complex types need nested structs
						let nested_prefix =
							format!("{}{}", struct_name_prefix, capitalize_first(field_name));
						let nested_fields = build_return_fields(
							ctx,
							return_type,
							&nested_info.traversal,
							&nested_prefix,
						);
						let nested_struct_name = format!("{}ReturnType", nested_prefix);
						let is_first =
							matches!(nested_info.traversal.should_collect, ShouldCollect::ToObj);

						// For variable references (empty source step), use empty string for traversal_code
						let traversal_code = if is_variable_reference {
							String::new()
						} else {
							nested_info.traversal.format_steps_only()
						};

						fields.push(ReturnFieldInfo {
							name: field_name.clone(),
							field_type: ReturnFieldType::Nested(nested_fields),
							source: ReturnFieldSource::NestedTraversal {
								traversal_expr: format!("nested_traversal_{}", field_name),
								traversal_code: Some(traversal_code),
								nested_struct_name: Some(nested_struct_name),
								traversal_type: Some(nested_info.traversal.traversal_type.clone()),
								closure_param_name: nested_info.closure_param_name.clone(),
								closure_source_var: nested_info.closure_source_var.clone(),
								accessed_field_name: None,
								own_closure_param: nested_info.own_closure_param.clone(),
								requires_full_traversal: nested_info.traversal.has_graph_steps(),
								is_first,
							},
						});
					}
				}
				_ => {
					// Other types - use placeholder
					fields.push(ReturnFieldInfo {
						name: field_name.clone(),
						field_type: ReturnFieldType::Simple(RustFieldType::Value),
						source: ReturnFieldSource::NestedTraversal {
							traversal_expr: format!("nested_traversal_{}", field_name),
							traversal_code: Some(nested_info.traversal.format_steps_only()),
							nested_struct_name: None,
							traversal_type: Some(nested_info.traversal.traversal_type.clone()),
							closure_param_name: nested_info.closure_param_name.clone(),
							closure_source_var: nested_info.closure_source_var.clone(),
							accessed_field_name: None,
							own_closure_param: nested_info.own_closure_param.clone(),
							requires_full_traversal: nested_info.traversal.has_graph_steps(),
							is_first: false,
						},
					});
				}
			}
		} else {
			// Type not yet determined - create placeholder
			// This will be filled in during a later pass
			fields.push(ReturnFieldInfo {
				name: field_name.clone(),
				field_type: ReturnFieldType::Simple(RustFieldType::Value),
				source: ReturnFieldSource::NestedTraversal {
					traversal_expr: format!("nested_traversal_{}", field_name),
					traversal_code: None,
					nested_struct_name: None,
					traversal_type: None,
					closure_param_name: None,
					closure_source_var: None,
					accessed_field_name: None,
					own_closure_param: None,
					requires_full_traversal: false,
					is_first: false,
				},
			});
		}
	}

	// Step 4: Add computed expression fields
	for (field_name, computed_info) in &traversal.computed_expressions {
		fields.push(ReturnFieldInfo {
			name: field_name.clone(),
			field_type: ReturnFieldType::Simple(RustFieldType::Value),
			source: ReturnFieldSource::ComputedExpression {
				expression: computed_info.expression.clone(),
			},
		});
	}

	fields
}

/// Helper function to get Rust type string from analyzer Type and populate return value fields
pub(crate) fn type_to_rust_string_and_fields(
	ty: &Type,
	should_collect: &ShouldCollect,
	ctx: &Ctx,
	_field_name: &str,
) -> (
	String,
	Vec<crate::helixc::generator::return_values::ReturnValueField>,
) {
	match (ty, should_collect) {
		// For single nodes/vectors/edges, generate a proper struct based on schema
		(Type::Node(Some(label)), ShouldCollect::ToObj | ShouldCollect::No) => {
			let type_name = format!("{}ReturnType", label);
			let mut fields = vec![
				crate::helixc::generator::return_values::ReturnValueField::new(
					"id".to_string(),
					"ID".to_string(),
				)
				.with_implicit(true),
				crate::helixc::generator::return_values::ReturnValueField::new(
					"label".to_string(),
					"String".to_string(),
				)
				.with_implicit(true),
			];

			// Add properties from schema (skip id and label as they're already added)
			if let Some(node_fields) = ctx.node_fields.get(label.as_str()) {
				for (prop_name, field) in node_fields {
					if *prop_name != "id" && *prop_name != "label" {
						fields.push(
							crate::helixc::generator::return_values::ReturnValueField::new(
								prop_name.to_string(),
								format!("{}", field.field_type),
							),
						);
					}
				}
			}
			(type_name, fields)
		}
		(Type::Edge(Some(label)), ShouldCollect::ToObj | ShouldCollect::No) => {
			let type_name = format!("{}ReturnType", label);
			let mut fields = vec![
				crate::helixc::generator::return_values::ReturnValueField::new(
					"id".to_string(),
					"ID".to_string(),
				)
				.with_implicit(true),
				crate::helixc::generator::return_values::ReturnValueField::new(
					"label".to_string(),
					"String".to_string(),
				)
				.with_implicit(true),
			];

			if let Some(edge_fields) = ctx.edge_fields.get(label.as_str()) {
				for (prop_name, field) in edge_fields {
					if *prop_name != "id" && *prop_name != "label" {
						fields.push(
							crate::helixc::generator::return_values::ReturnValueField::new(
								prop_name.to_string(),
								format!("{}", field.field_type),
							),
						);
					}
				}
			}
			(type_name, fields)
		}
		(Type::Vector(Some(label)), ShouldCollect::ToObj | ShouldCollect::No) => {
			let type_name = format!("{}ReturnType", label);
			let mut fields = vec![
				crate::helixc::generator::return_values::ReturnValueField::new(
					"id".to_string(),
					"ID".to_string(),
				)
				.with_implicit(true),
				crate::helixc::generator::return_values::ReturnValueField::new(
					"label".to_string(),
					"String".to_string(),
				)
				.with_implicit(true),
			];

			if let Some(vector_fields) = ctx.vector_fields.get(label.as_str()) {
				for (prop_name, field) in vector_fields {
					if *prop_name != "id" && *prop_name != "label" {
						fields.push(
							crate::helixc::generator::return_values::ReturnValueField::new(
								prop_name.to_string(),
								format!("{}", field.field_type),
							),
						);
					}
				}
			}
			(type_name, fields)
		}
		// For Vec types, we still need Vec<TypeName>
		(Type::Node(Some(label)), ShouldCollect::ToVec) => {
			(format!("Vec<{}ReturnType>", label), vec![])
		}
		(Type::Edge(Some(label)), ShouldCollect::ToVec) => {
			(format!("Vec<{}ReturnType>", label), vec![])
		}
		(Type::Vector(Some(label)), ShouldCollect::ToVec) => {
			(format!("Vec<{}ReturnType>", label), vec![])
		}
		// Fallbacks for None labels
		(Type::Node(None), _) | (Type::Edge(None), _) | (Type::Vector(None), _) => {
			("".to_string(), vec![])
		}
		(Type::Scalar(s), _) => (format!("{}", s), vec![]),
		(Type::Boolean, _) => ("bool".to_string(), vec![]),
		(Type::Array(inner), _) => {
			let (inner_type, _) =
				type_to_rust_string_and_fields(inner, &ShouldCollect::No, ctx, _field_name);
			(format!("Vec<{}>", inner_type), vec![])
		}
		(Type::Aggregate(_info), _) => {
			// For aggregates, return HashMap type since that's what group_by/aggregate_by returns
			// The actual struct fields will be generated later in build_return_fields
			("HashMap<String, AggregateItem>".to_string(), vec![])
		}
		_ => ("".to_string(), vec![]),
	}
}
