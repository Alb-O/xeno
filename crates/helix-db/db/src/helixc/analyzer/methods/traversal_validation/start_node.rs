use std::collections::HashMap;

use indexmap::IndexMap;
use paste::paste;

use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::{
	DEFAULT_VAR_NAME, VariableInfo, check_identifier_is_fieldtype, gen_identifier_or_param,
	is_valid_identifier, type_in_scope,
};
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::source_steps::{
	EFromID, EFromType, NFromID, NFromIndex, NFromType, SearchVector, SourceStep, VFromID,
	VFromType,
};
use crate::helixc::generator::traversal_steps::{
	ShouldCollect, Traversal as GeneratedTraversal, TraversalType,
};
use crate::helixc::generator::utils::{EmbedData, GenRef, GeneratedValue, Separator, VecData};
use crate::helixc::parser::types::*;
use crate::protocol::value::Value;

pub(crate) fn validate_start_node<'a>(
	ctx: &mut Ctx<'a>,
	tr: &'a Traversal,
	scope: &mut HashMap<&'a str, VariableInfo>,
	original_query: &'a Query,
	parent_ty: Option<Type>,
	gen_traversal: &mut GeneratedTraversal,
	gen_query: &mut GeneratedQuery,
) -> Option<Type> {
	match &tr.start {
		StartNode::Node { node_type, ids } => {
			if !ctx.node_set.contains(node_type.as_str()) {
				generate_error!(ctx, original_query, tr.loc.clone(), E101, node_type);
				return None;
			}
			if let Some(ids) = ids {
				assert!(ids.len() == 1, "multiple ids not supported yet");
				// check id exists in scope
				match ids.first().cloned() {
					Some(id) => {
						match id {
							IdType::ByIndex { index, value, loc } => {
								is_valid_identifier(
									ctx,
									original_query,
									loc.clone(),
									index.to_string().as_str(),
								);
								let corresponding_field = ctx
									.node_fields
									.get(node_type.as_str())
									.cloned()
									.ok_or_else(|| {
										generate_error!(
											ctx,
											original_query,
											loc.clone(),
											E201,
											node_type
										);
									})
									.unwrap_or_else(|_| {
										generate_error!(
											ctx,
											original_query,
											loc.clone(),
											E201,
											node_type
										);
										IndexMap::default()
									});

								match corresponding_field
									.iter()
									.find(|(name, _)| name.to_string() == *index.to_string())
								{
									Some((_, field)) => {
										if !field.is_indexed() {
											generate_error!(
												ctx,
												original_query,
												loc.clone(),
												E208,
												[&index.to_string(), node_type],
												[node_type]
											);
										} else if let ValueType::Literal { ref value, ref loc } =
											*value && !field.field_type.eq(value)
										{
											generate_error!(
												ctx,
												original_query,
												loc.clone(),
												E205,
												&value.inner_stringify(),
												&value.to_variant_string(),
												&field.field_type.to_string(),
												"node",
												node_type
											);
										}
									}
									None => {
										generate_error!(
											ctx,
											original_query,
											loc.clone(),
											E208,
											[&index.to_string(), node_type],
											[node_type]
										);
									}
								};
								gen_traversal.source_step =
									Separator::Period(SourceStep::NFromIndex(NFromIndex {
										label: GenRef::Literal(node_type.clone()),
										index: GenRef::Literal(match *index {
											IdType::Identifier { value, loc: _ } => value,
											// Parser guarantees index in ByIndex is always an Identifier
											_ => unreachable!(
												"parser guarantees index is Identifier"
											),
										}),
										key: match *value {
											ValueType::Identifier { value, loc } => {
												if is_valid_identifier(
													ctx,
													original_query,
													loc.clone(),
													value.as_str(),
												) && !scope.contains_key(value.as_str())
												{
													generate_error!(
														ctx,
														original_query,
														loc.clone(),
														E301,
														value.as_str()
													);
												}
												gen_identifier_or_param(
													original_query,
													value.as_str(),
													true,
													false,
												)
											}
											ValueType::Literal { value, loc: _ } => {
												GeneratedValue::Primitive(GenRef::Ref(
													match value {
														Value::String(s) => format!("\"{s}\""),
														other => other.inner_stringify(),
													},
												))
											}
											// Parser guarantees value in ByIndex is Identifier or Literal
											_ => unreachable!(
												"parser guarantees value is Identifier or Literal"
											),
										},
									}));
								gen_traversal.should_collect = ShouldCollect::ToObj;
								gen_traversal.traversal_type = TraversalType::Ref;
								Some(Type::Node(Some(node_type.to_string())))
							}
							IdType::Identifier { value: i, loc } => {
								gen_traversal.source_step =
									Separator::Period(SourceStep::NFromID(NFromID {
										id: {
											is_valid_identifier(
												ctx,
												original_query,
												loc.clone(),
												i.as_str(),
											);
											let _ = type_in_scope(
												ctx,
												original_query,
												loc.clone(),
												scope,
												i.as_str(),
											);
											let value = gen_identifier_or_param(
												original_query,
												i.as_str(),
												true,
												false,
											);
											check_identifier_is_fieldtype(
												ctx,
												original_query,
												loc.clone(),
												scope,
												i.as_str(),
												FieldType::Uuid,
											)?;
											value.inner().clone()
										},
										label: GenRef::Literal(node_type.clone()),
									}));
								gen_traversal.traversal_type = TraversalType::Ref;
								gen_traversal.should_collect = ShouldCollect::ToObj;
								Some(Type::Node(Some(node_type.to_string())))
							}
							IdType::Literal { value: s, loc: _ } => {
								gen_traversal.source_step =
									Separator::Period(SourceStep::NFromID(NFromID {
										id: GenRef::Ref(s.clone()),
										label: GenRef::Literal(node_type.clone()),
									}));
								gen_traversal.traversal_type = TraversalType::Ref;
								gen_traversal.should_collect = ShouldCollect::ToObj;
								Some(Type::Node(Some(node_type.to_string())))
							}
						}
					}
					None => {
						generate_error!(ctx, original_query, tr.loc.clone(), E601, "missing id");
						Some(Type::Unknown)
					}
				}
			} else {
				gen_traversal.source_step = Separator::Period(SourceStep::NFromType(NFromType {
					label: GenRef::Literal(node_type.clone()),
				}));
				gen_traversal.traversal_type = TraversalType::Ref;
				Some(Type::Nodes(Some(node_type.to_string())))
			}
		}
		StartNode::Edge { edge_type, ids } => {
			if !ctx.edge_map.contains_key(edge_type.as_str()) {
				generate_error!(ctx, original_query, tr.loc.clone(), E102, edge_type);
			}
			if let Some(ids) = ids {
				assert!(ids.len() == 1, "multiple ids not supported yet");
				gen_traversal.source_step = Separator::Period(SourceStep::EFromID(EFromID {
					id: match ids.first().cloned() {
						Some(id) => match id {
							IdType::Identifier { value: i, loc } => {
								is_valid_identifier(ctx, original_query, loc.clone(), i.as_str());
								let _ = type_in_scope(
									ctx,
									original_query,
									loc.clone(),
									scope,
									i.as_str(),
								);
								let value = gen_identifier_or_param(
									original_query,
									i.as_str(),
									true,
									false,
								);
								value.inner().clone()
							}
							IdType::Literal { value: s, loc: _ } => GenRef::Std(s),
							// Parser guarantees edge IDs are Identifier or Literal
							_ => unreachable!("parser guarantees edge ID is Identifier or Literal"),
						},
						None => {
							generate_error!(
								ctx,
								original_query,
								tr.loc.clone(),
								E601,
								"missing id"
							);
							GenRef::Unknown
						}
					},
					label: GenRef::Literal(edge_type.clone()),
				}));
				gen_traversal.traversal_type = TraversalType::Ref;
				gen_traversal.should_collect = ShouldCollect::ToObj;
				Some(Type::Edge(Some(edge_type.to_string())))
			} else {
				gen_traversal.source_step = Separator::Period(SourceStep::EFromType(EFromType {
					label: GenRef::Literal(edge_type.clone()),
				}));
				gen_traversal.traversal_type = TraversalType::Ref;
				Some(Type::Edges(Some(edge_type.to_string())))
			}
		}
		StartNode::Vector { vector_type, ids } => {
			if !ctx.vector_set.contains(vector_type.as_str()) {
				generate_error!(ctx, original_query, tr.loc.clone(), E103, vector_type);
			}
			if let Some(ids) = ids {
				assert!(ids.len() == 1, "multiple ids not supported yet");
				gen_traversal.source_step = Separator::Period(SourceStep::VFromID(VFromID {
					get_vector_data: false,
					id: match ids.first().cloned() {
						Some(id) => match id {
							IdType::Identifier { value: i, loc } => {
								is_valid_identifier(ctx, original_query, loc.clone(), i.as_str());
								let _ = type_in_scope(
									ctx,
									original_query,
									loc.clone(),
									scope,
									i.as_str(),
								);
								let value = gen_identifier_or_param(
									original_query,
									i.as_str(),
									true,
									false,
								);
								value.inner().clone()
							}
							IdType::Literal { value: s, loc: _ } => GenRef::Std(s),
							// Parser guarantees vector IDs are Identifier or Literal
							_ => {
								unreachable!("parser guarantees vector ID is Identifier or Literal")
							}
						},
						None => {
							generate_error!(
								ctx,
								original_query,
								tr.loc.clone(),
								E601,
								"missing id"
							);
							GenRef::Unknown
						}
					},
					label: GenRef::Literal(vector_type.clone()),
				}));
				gen_traversal.traversal_type = TraversalType::Ref;
				gen_traversal.should_collect = ShouldCollect::ToObj;
				Some(Type::Vector(Some(vector_type.to_string())))
			} else {
				gen_traversal.source_step = Separator::Period(SourceStep::VFromType(VFromType {
					label: GenRef::Literal(vector_type.clone()),
					get_vector_data: false,
				}));
				gen_traversal.traversal_type = TraversalType::Ref;
				Some(Type::Vectors(Some(vector_type.to_string())))
			}
		}

		StartNode::Identifier(identifier) => {
			match is_valid_identifier(ctx, original_query, tr.loc.clone(), identifier.as_str()) {
				true => {
					// Increment reference count for this variable
					if let Some(var_info) = scope.get_mut(identifier.as_str()) {
						var_info.increment_reference();

						// Mark traversal as reused if referenced more than once
						if var_info.reference_count > 1 {
							gen_traversal.is_reused_variable = true;
						}

						gen_traversal.traversal_type = if var_info.is_single {
							TraversalType::FromSingle(GenRef::Std(identifier.clone()))
						} else {
							TraversalType::FromIter(GenRef::Std(identifier.clone()))
						};
						gen_traversal.source_step = Separator::Empty(SourceStep::Identifier(
							GenRef::Std(identifier.clone()),
						));
						Some(var_info.ty.clone())
					} else {
						generate_error!(
							ctx,
							original_query,
							tr.loc.clone(),
							E301,
							identifier.as_str()
						);
						Some(Type::Unknown)
					}
				}
				false => Some(Type::Unknown),
			}
		}
		// anonymous will be the traversal type rather than the start type
		StartNode::Anonymous => {
			let Some(parent) = parent_ty.clone() else {
				generate_error!(
					ctx,
					original_query,
					tr.loc.clone(),
					E601,
					"anonymous traversal requires parent type"
				);
				return None;
			};
			gen_traversal.traversal_type =
				TraversalType::FromSingle(GenRef::Std(DEFAULT_VAR_NAME.to_string()));
			gen_traversal.source_step = Separator::Empty(SourceStep::Anonymous);
			Some(parent)
		}
		StartNode::SearchVector(sv) => {
			if let Some(ref ty) = sv.vector_type
				&& !ctx.vector_set.contains(ty.as_str())
			{
				generate_error!(ctx, original_query, sv.loc.clone(), E103, ty.as_str());
			}
			let vec: VecData = match &sv.data {
				Some(VectorData::Vector(v)) => {
					VecData::Standard(GeneratedValue::Literal(GenRef::Ref(format!(
						"[{}]",
						v.iter()
							.map(|f| f.to_string())
							.collect::<Vec<String>>()
							.join(",")
					))))
				}
				Some(VectorData::Identifier(i)) => {
					is_valid_identifier(ctx, original_query, sv.loc.clone(), i.as_str());
					// if is in params then use data.
					let _ = type_in_scope(ctx, original_query, sv.loc.clone(), scope, i.as_str());
					VecData::Standard(gen_identifier_or_param(
						original_query,
						i.as_str(),
						true,
						false,
					))
				}
				Some(VectorData::Embed(e)) => {
					let embed_data = match &e.value {
						EvaluatesToString::Identifier(i) => {
							let _ = type_in_scope(
								ctx,
								original_query,
								sv.loc.clone(),
								scope,
								i.as_str(),
							);
							EmbedData {
								data: gen_identifier_or_param(
									original_query,
									i.as_str(),
									true,
									false,
								),
								model_name: gen_query.embedding_model_to_use.clone(),
							}
						}
						EvaluatesToString::StringLiteral(s) => EmbedData {
							data: GeneratedValue::Literal(GenRef::Ref(s.clone())),
							model_name: gen_query.embedding_model_to_use.clone(),
						},
					};

					VecData::Hoisted(gen_query.add_hoisted_embed(embed_data))
				}
				_ => {
					generate_error!(
						ctx,
						original_query,
						sv.loc.clone(),
						E305,
						["vector_data", "SearchV"],
						["vector_data"]
					);
					VecData::Unknown
				}
			};
			let k = match &sv.k {
				Some(k) => match &k.value {
					EvaluatesToNumberType::I8(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					EvaluatesToNumberType::I16(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					EvaluatesToNumberType::I32(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					EvaluatesToNumberType::I64(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}

					EvaluatesToNumberType::U8(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					EvaluatesToNumberType::U16(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					EvaluatesToNumberType::U32(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					EvaluatesToNumberType::U64(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					EvaluatesToNumberType::U128(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					EvaluatesToNumberType::Identifier(i) => {
						let _ =
							is_valid_identifier(ctx, original_query, sv.loc.clone(), i.as_str());
						let _ =
							type_in_scope(ctx, original_query, sv.loc.clone(), scope, i.as_str());
						gen_identifier_or_param(original_query, i, false, true)
					}
					_ => {
						generate_error!(
							ctx,
							original_query,
							sv.loc.clone(),
							E305,
							["k", "SearchV"],
							["k"]
						);
						GeneratedValue::Unknown
					}
				},
				None => {
					generate_error!(ctx, original_query, sv.loc.clone(), E601, &sv.loc.span);
					GeneratedValue::Unknown
				}
			};

			let pre_filter = None;

			gen_traversal.traversal_type = TraversalType::Ref;
			gen_traversal.should_collect = ShouldCollect::ToVec;

			let label = match &sv.vector_type {
				Some(vt) => GenRef::Literal(vt.clone()),
				None => {
					generate_error!(
						ctx,
						original_query,
						sv.loc.clone(),
						E601,
						"search vector requires vector_type"
					);
					return None;
				}
			};

			gen_traversal.source_step = Separator::Period(SourceStep::SearchVector(SearchVector {
				label,
				vec,
				k,
				pre_filter,
			}));
			// Search returns nodes that contain the vectors
			Some(Type::Vectors(sv.vector_type.clone()))
		}
	}
}
