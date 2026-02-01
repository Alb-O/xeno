use std::collections::HashMap;

use paste::paste;

use super::return_fields::{build_return_fields, type_to_rust_string_and_fields};
use super::utils::capitalize_first;
use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::methods::infer_expr_type::infer_expr_type;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::{VariableInfo, is_valid_identifier};
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::return_values::{ReturnValue, ReturnValueStruct};
use crate::helixc::generator::source_steps::SourceStep;
use crate::helixc::generator::statements::Statement as GeneratedStatement;
use crate::helixc::generator::traversal_steps::{
	ShouldCollect, Traversal as GeneratedTraversal, TraversalType,
};
use crate::helixc::generator::utils::GenRef;
use crate::helixc::parser::types::*;

pub(crate) fn analyze_return_expr<'a>(
	ctx: &mut Ctx<'a>,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	query: &mut GeneratedQuery,
	ret: &'a ReturnType,
) {
	match ret {
		ReturnType::Expression(expr) => {
			let (inferred_type, stmt) =
				infer_expr_type(ctx, expr, scope, original_query, None, query);

			if stmt.is_none() {
				return;
			}

			match stmt.unwrap() {
				GeneratedStatement::Traversal(traversal) => {
					match &traversal.source_step.inner() {
						SourceStep::Identifier(v) => {
							is_valid_identifier(
								ctx,
								original_query,
								expr.loc.clone(),
								v.inner().as_str(),
							);

							let field_name = v.inner().clone();

							// Legacy approach
							let (rust_type, fields) = type_to_rust_string_and_fields(
								&inferred_type,
								&traversal.should_collect,
								ctx,
								&field_name,
							);

							// For Scalar types with field access (e.g., dataset_id::{value} or files::ID),
							// generate the property access code
							let literal_value = if matches!(inferred_type, Type::Scalar(_))
								&& !traversal.object_fields.is_empty()
							{
								let property_name = &traversal.object_fields[0];

								match traversal.should_collect {
									ShouldCollect::ToObj => {
										// Single item - use literal_value
										if property_name == "id" {
											Some(GenRef::Std(format!(
												"uuid_str({}.id(), &arena)",
												field_name
											)))
										} else if property_name == "label" {
											Some(GenRef::Std(format!("{}.label()", field_name)))
										} else {
											Some(GenRef::Std(format!(
												"{}.get_property(\"{}\")",
												field_name, property_name
											)))
										}
									}
									ShouldCollect::ToVec => {
										// Collection - generate iteration code
										let iter_code = if property_name == "id" {
											format!(
												"{}.iter().map(|item| uuid_str(item.id(), &arena)).collect::<Vec<_>>()",
												field_name
											)
										} else if property_name == "label" {
											format!(
												"{}.iter().map(|item| item.label()).collect::<Vec<_>>()",
												field_name
											)
										} else {
											format!(
												"{}.iter().map(|item| item.get_property(\"{}\")).collect::<Vec<_>>()",
												field_name, property_name
											)
										};
										Some(GenRef::Std(iter_code))
									}
									_ => None,
								}
							} else {
								None
							};

							query.return_values.push((
								field_name.clone(),
								ReturnValue {
									name: rust_type,
									fields,
									literal_value: literal_value.clone(),
								},
							));

							// New unified approach
							if matches!(
								inferred_type,
								Type::Boolean | Type::Scalar(_) | Type::Count
							) {
								// Primitive types: emit variable directly, no struct needed
								let mut prim_struct = ReturnValueStruct::new(field_name.clone());
								prim_struct.source_variable = field_name.clone();
								prim_struct.is_primitive = true;
								prim_struct.primitive_literal_value = literal_value;
								query.return_structs.push(prim_struct);
							} else {
								let struct_name_prefix = format!(
									"{}{}",
									capitalize_first(&query.name),
									capitalize_first(&field_name)
								);
								let return_fields = build_return_fields(
									ctx,
									&inferred_type,
									&traversal,
									&struct_name_prefix,
								);
								let struct_name = format!("{}ReturnType", struct_name_prefix);
								let is_collection = matches!(
									inferred_type,
									Type::Nodes(_) | Type::Edges(_) | Type::Vectors(_)
								);
								let (
									is_aggregate,
									is_group_by,
									aggregate_properties,
									is_count_aggregate,
								) = match inferred_type {
									Type::Aggregate(info) => (
										true,
										info.is_group_by,
										info.properties.clone(),
										info.is_count,
									),
									_ => (false, false, Vec::new(), false),
								};
								query
									.return_structs
									.push(ReturnValueStruct::from_return_fields(
										struct_name.clone(),
										return_fields.clone(),
										field_name.clone(),
										is_collection,
										traversal.is_reused_variable,
										is_aggregate,
										is_group_by,
										aggregate_properties,
										is_count_aggregate,
										traversal.closure_param_name.clone(),
									));
							}
						}
						_ => {
							let field_name = "data".to_string();

							// Legacy approach
							let (rust_type, fields) = type_to_rust_string_and_fields(
								&inferred_type,
								&traversal.should_collect,
								ctx,
								&field_name,
							);
							query.return_values.push((
								field_name.clone(),
								ReturnValue {
									name: rust_type,
									fields,
									literal_value: None,
								},
							));

							// New unified approach
							if matches!(
								inferred_type,
								Type::Boolean | Type::Scalar(_) | Type::Count
							) {
								let mut prim_struct = ReturnValueStruct::new(field_name.clone());
								prim_struct.source_variable = field_name.clone();
								prim_struct.is_primitive = true;
								query.return_structs.push(prim_struct);
							} else {
								let struct_name_prefix = format!(
									"{}{}",
									capitalize_first(&query.name),
									capitalize_first(&field_name)
								);
								let return_fields = build_return_fields(
									ctx,
									&inferred_type,
									&traversal,
									&struct_name_prefix,
								);
								let struct_name = format!("{}ReturnType", struct_name_prefix);
								let is_collection = matches!(
									inferred_type,
									Type::Nodes(_) | Type::Edges(_) | Type::Vectors(_)
								);
								let (
									is_aggregate,
									is_group_by,
									aggregate_properties,
									is_count_aggregate,
								) = match inferred_type {
									Type::Aggregate(info) => (
										true,
										info.is_group_by,
										info.properties.clone(),
										info.is_count,
									),
									_ => (false, false, Vec::new(), false),
								};
								query
									.return_structs
									.push(ReturnValueStruct::from_return_fields(
										struct_name.clone(),
										return_fields.clone(),
										field_name.clone(),
										is_collection,
										traversal.is_reused_variable,
										is_aggregate,
										is_group_by,
										aggregate_properties,
										is_count_aggregate,
										traversal.closure_param_name.clone(),
									));
							}
						}
					}
				}
				GeneratedStatement::Identifier(id) => {
					is_valid_identifier(ctx, original_query, expr.loc.clone(), id.inner().as_str());
					let identifier_end_type = match scope.get(id.inner().as_str()) {
						Some(var_info) => var_info.ty.clone(),
						None => {
							generate_error!(
								ctx,
								original_query,
								expr.loc.clone(),
								E301,
								id.inner().as_str()
							);
							Type::Unknown
						}
					};

					let field_name = id.inner().clone();

					// Legacy approach
					let (rust_type, fields) = type_to_rust_string_and_fields(
						&identifier_end_type,
						&ShouldCollect::No,
						ctx,
						&field_name,
					);
					query.return_values.push((
						field_name.clone(),
						ReturnValue {
							name: rust_type,
							fields,
							literal_value: None,
						},
					));

					// New unified approach
					if matches!(
						identifier_end_type,
						Type::Boolean | Type::Scalar(_) | Type::Count
					) {
						// Primitive types: emit variable directly, no struct needed
						let mut prim_struct = ReturnValueStruct::new(field_name.clone());
						prim_struct.source_variable = field_name.clone();
						prim_struct.is_primitive = true;
						query.return_structs.push(prim_struct);
					} else {
						// For identifier returns, we need to create a traversal to build fields from
						let var_info = scope.get(id.inner().as_str());
						let is_reused = var_info.map(|v| v.reference_count > 1).unwrap_or(false);

						let traversal = GeneratedTraversal {
							traversal_type: TraversalType::Ref,
							is_reused_variable: is_reused,
							..Default::default()
						};

						let struct_name_prefix = format!(
							"{}{}",
							capitalize_first(&query.name),
							capitalize_first(&field_name)
						);
						let return_fields = build_return_fields(
							ctx,
							&identifier_end_type,
							&traversal,
							&struct_name_prefix,
						);
						let struct_name = format!("{}ReturnType", struct_name_prefix);
						let is_collection = matches!(
							identifier_end_type,
							Type::Nodes(_) | Type::Edges(_) | Type::Vectors(_)
						);
						let (is_aggregate, is_group_by, aggregate_properties, is_count_aggregate) =
							match identifier_end_type {
								Type::Aggregate(info) => (
									true,
									info.is_group_by,
									info.properties.clone(),
									info.is_count,
								),
								_ => (false, false, Vec::new(), false),
							};
						query
							.return_structs
							.push(ReturnValueStruct::from_return_fields(
								struct_name.clone(),
								return_fields.clone(),
								field_name.clone(),
								is_collection,
								is_reused,
								is_aggregate,
								is_group_by,
								aggregate_properties,
								is_count_aggregate,
								None,
							));
					}
				}
				_ => {}
			}
		}
		ReturnType::Object(object_fields) => {
			let struct_name = format!("{}ReturnType", capitalize_first(&query.name));
			let _ = process_object_literal(
				ctx,
				original_query,
				scope,
				query,
				object_fields,
				struct_name,
			);
		}
		_ => {}
	}
}

pub(crate) fn process_object_literal<'a>(
	ctx: &mut Ctx<'a>,
	_original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	query: &mut GeneratedQuery,
	object_fields: &HashMap<String, ReturnType>,
	_struct_name: String,
) -> ReturnValueStruct {
	// Build JSON construction code recursively
	fn build_json_code<'a>(
		ctx: &Ctx<'a>,
		obj_fields: &HashMap<String, ReturnType>,
		scope: &HashMap<&str, VariableInfo>,
	) -> String {
		let mut json_parts = Vec::new();

		for (field_name, return_type) in obj_fields {
			let field_value = match return_type {
				ReturnType::Expression(expr) => match &expr.expr {
					ExpressionType::Traversal(trav) => {
						let var_name = match &trav.start {
							StartNode::Identifier(id) => id.clone(),
							_ => "unknown".to_string(),
						};

						if let Some(step) = trav.steps.first() {
							if let StepType::Object(obj) = &step.step {
								if let Some(field) = obj.fields.first() {
									let prop_name = &field.key;

									if prop_name == "id" {
										format!("uuid_str({}.id(), &arena)", var_name)
									} else if prop_name == "label" {
										format!("{}.label()", var_name)
									} else {
										format!("{}.get_property(\"{}\")", var_name, prop_name)
									}
								} else {
									format!("json!({})", var_name)
								}
							} else {
								format!("json!({})", var_name)
							}
						} else {
							format!("json!({})", var_name)
						}
					}
					ExpressionType::Identifier(id) => {
						if let Some(var_info) = scope.get(id.as_str()) {
							build_identifier_json(ctx, id, &var_info.ty)
						} else {
							format!("json!({})", id)
						}
					}
					_ => "serde_json::Value::Null".to_string(),
				},
				ReturnType::Object(nested_obj) => {
					let nested_json = build_json_code(ctx, nested_obj, scope);
					format!("json!({})", nested_json)
				}
				ReturnType::Array(arr) => {
					let mut array_parts = Vec::new();
					for elem in arr {
						match elem {
							ReturnType::Expression(expr) => match &expr.expr {
								ExpressionType::Identifier(id) => {
									if let Some(var_info) = scope.get(id.as_str()) {
										array_parts.push(build_identifier_json(
											ctx,
											id,
											&var_info.ty,
										));
									} else {
										array_parts.push(format!("json!({})", id));
									}
								}
								ExpressionType::Traversal(trav) => {
									let var_name = match &trav.start {
										StartNode::Identifier(id) => id.clone(),
										_ => "unknown".to_string(),
									};

									if let Some(step) = trav.steps.first() {
										if let StepType::Object(obj) = &step.step {
											if let Some(field) = obj.fields.first() {
												let prop_name = &field.key;
												if prop_name == "id" {
													array_parts.push(format!(
														"uuid_str({}.id(), &arena)",
														var_name
													));
												} else if prop_name == "label" {
													array_parts
														.push(format!("{}.label()", var_name));
												} else {
													array_parts.push(format!(
														"{}.get_property(\"{}\")",
														var_name, prop_name
													));
												}
											} else {
												array_parts.push(format!("json!({})", var_name));
											}
										} else {
											array_parts.push(format!("json!({})", var_name));
										}
									} else {
										array_parts.push(format!("json!({})", var_name));
									}
								}
								_ => {
									array_parts.push("serde_json::Value::Null".to_string());
								}
							},
							ReturnType::Object(obj) => {
								let nested_json = build_json_code(ctx, obj, scope);
								array_parts.push(format!("json!({})", nested_json));
							}
							_ => {
								array_parts.push("serde_json::Value::Null".to_string());
							}
						}
					}
					format!("json!([{}])", array_parts.join(", "))
				}
				ReturnType::Empty => "serde_json::Value::Null".to_string(),
			};

			json_parts.push(format!("\"{}\": {}", field_name, field_value));
		}

		format!("{{\n        {}\n    }}", json_parts.join(",\n        "))
	}

	// Helper function to build JSON for an identifier based on its type
	fn build_identifier_json(ctx: &Ctx, var_name: &str, ty: &Type) -> String {
		match ty {
			Type::Node(Some(label)) => {
				if let Some(node_fields) = ctx.node_fields.get(label.as_str()) {
					let mut props = vec![
						format!("\"id\": uuid_str({}.id(), &arena)", var_name),
						format!("\"label\": {}.label()", var_name),
					];

					for (prop_name, _prop_type) in node_fields.iter() {
						if *prop_name == "id" || *prop_name == "label" {
							continue;
						}
						props.push(format!(
							"\"{}\":  {}.get_property(\"{}\")",
							prop_name, var_name, prop_name
						));
					}

					format!("json!({{\n        {}\n    }})", props.join(",\n        "))
				} else {
					format!(
						"json!({{\"id\": uuid_str({}.id(), &arena), \"label\": {}.label()}})",
						var_name, var_name
					)
				}
			}
			Type::Edge(Some(label)) => {
				if let Some(edge_fields) = ctx.edge_fields.get(label.as_str()) {
					let mut props = vec![
						format!("\"id\": uuid_str({}.id(), &arena)", var_name),
						format!("\"label\": {}.label()", var_name),
					];

					for (prop_name, _prop_type) in edge_fields.iter() {
						if *prop_name == "id" || *prop_name == "label" {
							continue;
						}
						props.push(format!(
							"\"{}\":  {}.get_property(\"{}\")",
							prop_name, var_name, prop_name
						));
					}

					format!("json!({{\n        {}\n    }})", props.join(",\n        "))
				} else {
					format!(
						"json!({{\"id\": uuid_str({}.id(), &arena), \"label\": {}.label()}})",
						var_name, var_name
					)
				}
			}
			_ => {
				format!("json!({})", var_name)
			}
		}
	}

	let json_code = build_json_code(ctx, object_fields, scope);

	query.return_values.push((
		"response".to_string(),
		ReturnValue {
			name: "serde_json::Value".to_string(),
			fields: vec![],
			literal_value: Some(GenRef::Std(format!("json!({})", json_code))),
		},
	));

	query.use_struct_returns = false;

	ReturnValueStruct {
		name: "Unused".to_string(),
		fields: vec![],
		has_lifetime: false,
		is_query_return_type: false,
		is_collection: false,
		is_aggregate: false,
		is_group_by: false,
		source_variable: String::new(),
		is_reused_variable: false,
		is_primitive: false,
		field_infos: vec![],
		aggregate_properties: Vec::new(),
		is_count_aggregate: false,
		closure_param_name: None,
		primitive_literal_value: None,
	}
}
