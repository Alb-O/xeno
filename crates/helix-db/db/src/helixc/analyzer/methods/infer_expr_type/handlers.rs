use std::collections::HashMap;

use paste::paste;

use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::{
	VariableInfo, gen_id_access_or_param, gen_identifier_or_param, is_valid_identifier,
	type_in_scope, validate_id_type,
};
use crate::helixc::generator::bool_ops::BoExp;
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::source_steps::{
	AddE, AddN, AddV, SearchVector as GeneratedSearchVector, SourceStep,
};
use crate::helixc::generator::statements::Statement as GeneratedStatement;
use crate::helixc::generator::traversal_steps::{
	ShouldCollect, Step as GeneratedStep, Traversal as GeneratedTraversal, TraversalType, Where,
	WhereRef,
};
use crate::helixc::generator::utils::{EmbedData, GenRef, GeneratedValue, Separator, VecData};
use crate::helixc::parser::types::*;
use crate::protocol::date::Date;

pub(crate) fn handle_add_node<'a>(
	ctx: &mut Ctx<'a>,
	add: &'a AddNode,
	scope: &mut HashMap<&'a str, VariableInfo>,
	original_query: &'a Query,
	gen_query: &mut GeneratedQuery,
) -> (Type, Option<GeneratedStatement>) {
	if let Some(ref ty) = add.node_type {
		if !ctx.node_set.contains(ty.as_str()) {
			generate_error!(ctx, original_query, add.loc.clone(), E101, ty.as_str());
		}
		let label = GenRef::Literal(ty.clone());

		let node_in_schema = match ctx.output.nodes.iter().find(|n| n.name == ty.as_str()) {
			Some(node) => node.clone(),
			None => {
				generate_error!(ctx, original_query, add.loc.clone(), E101, ty.as_str());
				return (Type::Node(None), None);
			}
		};

		let default_properties = node_in_schema
			.properties
			.iter()
			.filter_map(|p| p.default_value.clone().map(|v| (p.name.clone(), v)))
			.collect::<Vec<(String, GeneratedValue)>>();

		let (properties, secondary_indices) = match &add.fields {
			Some(fields_to_add) => {
				let field_set_from_schema = ctx.node_fields.get(ty.as_str()).cloned();
				if let Some(field_set) = field_set_from_schema {
					for (field_name, field_value) in fields_to_add {
						if !field_set.contains_key(field_name.as_str()) {
							generate_error!(
								ctx,
								original_query,
								add.loc.clone(),
								E202,
								field_name.as_str(),
								"node",
								ty.as_str()
							);
						}
						match field_value {
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
								} else {
									if let Some(var_info) = scope.get(value.as_str())
										&& let Some(field) = field_set.get(field_name.as_str())
									{
										let variable_type = &var_info.ty;
										if variable_type != &Type::from(field.field_type.clone()) {
											generate_error!(
												ctx,
												original_query,
												loc.clone(),
												E205,
												value.as_str(),
												&variable_type.to_string(),
												&field.field_type.to_string(),
												"node",
												ty.as_str()
											);
										}
									}
								}
							}
							ValueType::Literal { value, loc } => {
								if let Some(fields) = ctx.node_fields.get(ty.as_str())
									&& let Some(field) = fields.get(field_name.as_str())
								{
									let field_type = field.field_type.clone();
									if field_type != *value {
										generate_error!(
											ctx,
											original_query,
											loc.clone(),
											E205,
											&value.inner_stringify(),
											value.to_variant_string(),
											&field_type.to_string(),
											"node",
											ty.as_str()
										);
									}
								}
							}
							_ => {}
						}
					}
				}
				let mut properties = fields_to_add
					.iter()
					.map(|(field_name, value)| {
						(
							field_name.clone(),
							match value {
								ValueType::Literal { value, loc } => {
									match ctx.node_fields.get(ty.as_str()) {
										Some(fields) => match fields.get(field_name.as_str()) {
											Some(field) => {
												match field.field_type == FieldType::Date {
													true => match Date::new(value) {
														Ok(date) => GeneratedValue::Literal(
															GenRef::Literal(date.to_rfc3339()),
														),
														Err(_) => {
															generate_error!(
																ctx,
																original_query,
																loc.clone(),
																E501,
																value.as_str()
															);
															GeneratedValue::Unknown
														}
													},
													false => GeneratedValue::Literal(GenRef::from(
														value.clone(),
													)),
												}
											}
											None => GeneratedValue::Unknown,
										},
										None => GeneratedValue::Unknown,
									}
								}
								ValueType::Identifier { value, .. } => {
									gen_identifier_or_param(original_query, value, true, false)
								}
								v => {
									generate_error!(
										ctx,
										original_query,
										add.loc.clone(),
										E206,
										&v.to_string()
									);
									GeneratedValue::Unknown
								}
							},
						)
					})
					.collect::<HashMap<String, GeneratedValue>>();

				for (field_name, default_value) in default_properties {
					if !properties.contains_key(field_name.as_str()) {
						properties.insert(field_name, default_value);
					}
				}

				let secondary_indices = {
					let secondary_indices = node_in_schema
						.properties
						.iter()
						.filter_map(|p| p.field_prefix.is_indexed().then_some(p.name.clone()))
						.collect::<Vec<_>>();
					match secondary_indices.is_empty() {
						true => None,
						false => Some(secondary_indices),
					}
				};

				(properties, secondary_indices)
			}
			None => (
				default_properties.into_iter().fold(
					HashMap::new(),
					|mut acc, (field_name, default_value)| {
						acc.insert(field_name, default_value);
						acc
					},
				),
				None,
			),
		};

		let add_n = AddN {
			label,
			properties: Some(properties.into_iter().collect()),
			secondary_indices,
		};

		let stmt = GeneratedStatement::Traversal(GeneratedTraversal {
			source_step: Separator::Period(SourceStep::AddN(add_n)),
			steps: vec![],
			traversal_type: TraversalType::Mut,
			should_collect: ShouldCollect::ToObj,
			..Default::default()
		});
		gen_query.is_mut = true;
		return (Type::Node(Some(ty.to_string())), Some(stmt));
	}
	generate_error!(
		ctx,
		original_query,
		add.loc.clone(),
		E304,
		["node"],
		["node"]
	);
	(Type::Node(None), None)
}

pub(crate) fn handle_add_edge<'a>(
	ctx: &mut Ctx<'a>,
	add: &'a AddEdge,
	scope: &mut HashMap<&'a str, VariableInfo>,
	original_query: &'a Query,
	gen_query: &mut GeneratedQuery,
) -> (Type, Option<GeneratedStatement>) {
	if let Some(ref ty) = add.edge_type {
		if !ctx.edge_map.contains_key(ty.as_str()) {
			generate_error!(ctx, original_query, add.loc.clone(), E102, ty.as_str());
		}
		let label = GenRef::Literal(ty.clone());

		let edge_in_schema = match ctx.output.edges.iter().find(|e| e.name == ty.as_str()) {
			Some(edge) => edge.clone(),
			None => {
				generate_error!(ctx, original_query, add.loc.clone(), E102, ty.as_str());
				return (Type::Edge(None), None);
			}
		};

		let default_properties = edge_in_schema
			.properties
			.iter()
			.filter_map(|p| p.default_value.clone().map(|v| (p.name.clone(), v)))
			.collect::<Vec<(String, GeneratedValue)>>();

		let properties = match &add.fields {
			Some(fields) => {
				let field_set = ctx.edge_fields.get(ty.as_str()).cloned();
				if let Some(field_set) = field_set {
					for (field_name, value) in fields {
						if !field_set.contains_key(field_name.as_str()) {
							generate_error!(
								ctx,
								original_query,
								add.loc.clone(),
								E202,
								field_name.as_str(),
								"edge",
								ty.as_str()
							);
						}

						match value {
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
								} else {
									if let Some(var_info) = scope.get(value.as_str())
										&& let Some(field) = field_set.get(field_name.as_str())
									{
										let variable_type = &var_info.ty;
										if variable_type != &Type::from(field.field_type.clone()) {
											generate_error!(
												ctx,
												original_query,
												loc.clone(),
												E205,
												value.as_str(),
												&variable_type.to_string(),
												&field.field_type.to_string(),
												"edge",
												ty.as_str()
											);
										}
									}
								}
							}
							ValueType::Literal { value, loc } => {
								if let Some(fields) = ctx.edge_fields.get(ty.as_str())
									&& let Some(field) = fields.get(field_name.as_str())
								{
									let field_type = field.field_type.clone();
									if field_type != *value {
										generate_error!(
											ctx,
											original_query,
											loc.clone(),
											E205,
											&value.inner_stringify(),
											value.to_variant_string(),
											&field_type.to_string(),
											"edge",
											ty.as_str()
										);
									}
								}
							}
							_ => {}
						}
					}
				}
				let mut properties = fields
					.iter()
					.map(|(field_name, value)| {
						(
							field_name.clone(),
							match value {
								ValueType::Literal { value, loc } => {
									match ctx.edge_fields.get(ty.as_str()) {
										Some(fields) => match fields.get(field_name.as_str()) {
											Some(field) => {
												match field.field_type == FieldType::Date {
													true => match Date::new(value) {
														Ok(date) => GeneratedValue::Literal(
															GenRef::Literal(date.to_rfc3339()),
														),
														Err(_) => {
															generate_error!(
																ctx,
																original_query,
																loc.clone(),
																E501,
																value.as_str()
															);
															GeneratedValue::Unknown
														}
													},
													false => GeneratedValue::Literal(GenRef::from(
														value.clone(),
													)),
												}
											}
											None => GeneratedValue::Unknown,
										},
										None => GeneratedValue::Unknown,
									}
								}
								ValueType::Identifier { value, loc } => {
									is_valid_identifier(
										ctx,
										original_query,
										loc.clone(),
										value.as_str(),
									);
									gen_identifier_or_param(
										original_query,
										value.as_str(),
										false,
										true,
									)
								}
								v => {
									generate_error!(
										ctx,
										original_query,
										add.loc.clone(),
										E206,
										&v.to_string()
									);
									GeneratedValue::Unknown
								}
							},
						)
					})
					.collect::<HashMap<String, GeneratedValue>>();

				for (field_name, default_value) in default_properties.clone() {
					if !properties.contains_key(field_name.as_str()) {
						properties.insert(field_name, default_value);
					}
				}

				Some(properties.into_iter().collect())
			}
			None => match default_properties.is_empty() {
				true => None,
				false => Some(default_properties),
			},
		};

		let (to, to_is_plural) = match &add.connection.to_id {
			Some(id) => match id {
				IdType::Identifier { value, loc } => {
					is_valid_identifier(ctx, original_query, loc.clone(), value.as_str());
					if !scope.contains_key(value.as_str())
						&& crate::helixc::analyzer::utils::is_param(original_query, value.as_str())
							.is_none()
					{
						generate_error!(ctx, original_query, loc.clone(), E301, value.as_str());
					}
					validate_id_type(ctx, original_query, loc.clone(), scope, value.as_str());
					let is_plural = scope
						.get(value.as_str())
						.map(|var_info| !var_info.is_single)
						.unwrap_or(false);
					let gen_value = if is_plural {
						if let Some(param) =
							crate::helixc::analyzer::utils::is_param(original_query, value.as_str())
						{
							GeneratedValue::Parameter(match param.is_optional {
								true => GenRef::DeRef(format!(
									"data.{}.as_ref().ok_or_else(|| TraversalError::ParamNotFound(\"{}\").into())?",
									value, value
								)),
								false => GenRef::DeRef(format!("data.{}", value)),
							})
						} else {
							GeneratedValue::Identifier(GenRef::Std(value.clone()))
						}
					} else {
						gen_id_access_or_param(original_query, value.as_str())
					};
					(gen_value, is_plural)
				}
				IdType::Literal { value, loc: _ } => (
					GeneratedValue::Literal(GenRef::Literal(value.clone())),
					false,
				),
				_ => unreachable!("parser guarantees edge to_id is Identifier or Literal"),
			},
			_ => {
				generate_error!(ctx, original_query, add.loc.clone(), E611);
				(GeneratedValue::Unknown, false)
			}
		};
		let (from, from_is_plural) = match &add.connection.from_id {
			Some(id) => match id {
				IdType::Identifier { value, loc } => {
					is_valid_identifier(ctx, original_query, loc.clone(), value.as_str());
					if !scope.contains_key(value.as_str())
						&& crate::helixc::analyzer::utils::is_param(original_query, value.as_str())
							.is_none()
					{
						generate_error!(ctx, original_query, loc.clone(), E301, value.as_str());
					}
					validate_id_type(ctx, original_query, loc.clone(), scope, value.as_str());
					let is_plural = scope
						.get(value.as_str())
						.map(|var_info| !var_info.is_single)
						.unwrap_or(false);
					let gen_value = if is_plural {
						if let Some(param) =
							crate::helixc::analyzer::utils::is_param(original_query, value.as_str())
						{
							GeneratedValue::Parameter(match param.is_optional {
								true => GenRef::DeRef(format!(
									"data.{}.as_ref().ok_or_else(|| TraversalError::ParamNotFound(\"{}\").into())?",
									value, value
								)),
								false => GenRef::DeRef(format!("data.{}", value)),
							})
						} else {
							GeneratedValue::Identifier(GenRef::Std(value.clone()))
						}
					} else {
						gen_id_access_or_param(original_query, value.as_str())
					};
					(gen_value, is_plural)
				}
				IdType::Literal { value, loc: _ } => (
					GeneratedValue::Literal(GenRef::Literal(value.clone())),
					false,
				),
				_ => {
					unreachable!("parser guarantees edge from_id is Identifier or Literal")
				}
			},
			_ => {
				generate_error!(ctx, original_query, add.loc.clone(), E612);
				(GeneratedValue::Unknown, false)
			}
		};
		let add_e = AddE {
			to,
			from,
			label,
			properties,
			from_is_plural,
			to_is_plural,
			is_unique: edge_in_schema.is_unique,
		};
		let (final_traversal_result_type, traversal_type, separator, should_collect) =
			if from_is_plural || to_is_plural {
				(
					Type::Edges(Some(ty.to_string())),
					TraversalType::Standalone,
					Separator::Empty(SourceStep::AddE(add_e)),
					ShouldCollect::No,
				)
			} else {
				(
					Type::Edge(Some(ty.to_string())),
					TraversalType::Mut,
					Separator::Period(SourceStep::AddE(add_e)),
					ShouldCollect::ToObj,
				)
			};
		let stmt = GeneratedStatement::Traversal(GeneratedTraversal {
			source_step: separator,
			steps: vec![],
			traversal_type,
			should_collect,
			..Default::default()
		});
		gen_query.is_mut = true;
		return (final_traversal_result_type, Some(stmt));
	}
	generate_error!(
		ctx,
		original_query,
		add.loc.clone(),
		E304,
		["edge"],
		["edge"]
	);
	(Type::Edge(None), None)
}

pub(crate) fn handle_add_vector<'a>(
	ctx: &mut Ctx<'a>,
	add: &'a AddVector,
	scope: &mut HashMap<&'a str, VariableInfo>,
	original_query: &'a Query,
	gen_query: &mut GeneratedQuery,
) -> (Type, Option<GeneratedStatement>) {
	if let Some(ref ty) = add.vector_type {
		if !ctx.vector_set.contains(ty.as_str()) {
			generate_error!(ctx, original_query, add.loc.clone(), E103, ty.as_str());
		}
		let label = GenRef::Literal(ty.clone());

		let vector_in_schema = match ctx.output.vectors.iter().find(|v| v.name == ty.as_str()) {
			Some(vector) => vector.clone(),
			None => {
				generate_error!(ctx, original_query, add.loc.clone(), E103, ty.as_str());
				return (Type::Vector(None), None);
			}
		};

		let default_properties = vector_in_schema
			.properties
			.iter()
			.filter_map(|p| p.default_value.clone().map(|v| (p.name.clone(), v)))
			.collect::<Vec<(String, GeneratedValue)>>();

		let properties = match &add.fields {
			Some(fields) => {
				let field_set = ctx.vector_fields.get(ty.as_str()).cloned();
				if let Some(field_set) = field_set {
					for (field_name, value) in fields {
						if !field_set.contains_key(field_name.as_str()) {
							generate_error!(
								ctx,
								original_query,
								add.loc.clone(),
								E202,
								field_name.as_str(),
								"vector",
								ty.as_str()
							);
						}
						match value {
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
								} else {
									if let Some(var_info) = scope.get(value.as_str())
										&& let Some(field) = field_set.get(field_name.as_str())
									{
										let variable_type = &var_info.ty;
										if variable_type != &Type::from(field.field_type.clone()) {
											generate_error!(
												ctx,
												original_query,
												loc.clone(),
												E205,
												value.as_str(),
												&variable_type.to_string(),
												&field.field_type.to_string(),
												"vector",
												ty.as_str()
											);
										}
									}
								}
							}
							ValueType::Literal { value, loc } => {
								if let Some(fields) = ctx.vector_fields.get(ty.as_str())
									&& let Some(field) = fields.get(field_name.as_str())
								{
									let field_type = field.field_type.clone();
									if field_type != *value {
										generate_error!(
											ctx,
											original_query,
											loc.clone(),
											E205,
											value.as_str(),
											&value.to_variant_string(),
											&field_type.to_string(),
											"vector",
											ty.as_str()
										);
									}
								}
							}
							_ => {}
						}
					}
				}
				let mut properties = fields
					.iter()
					.map(|(field_name, value)| {
						(
							field_name.clone(),
							match value {
								ValueType::Literal { value, loc } => {
									match ctx.vector_fields.get(ty.as_str()) {
										Some(fields) => match fields.get(field_name.as_str()) {
											Some(field) => {
												match field.field_type == FieldType::Date {
													true => match Date::new(value) {
														Ok(date) => GeneratedValue::Literal(
															GenRef::Literal(date.to_rfc3339()),
														),
														Err(_) => {
															generate_error!(
																ctx,
																original_query,
																loc.clone(),
																E501,
																value.as_str()
															);
															GeneratedValue::Unknown
														}
													},
													false => GeneratedValue::Literal(GenRef::from(
														value.clone(),
													)),
												}
											}
											None => GeneratedValue::Unknown,
										},
										None => GeneratedValue::Unknown,
									}
								}
								ValueType::Identifier { value, loc } => {
									is_valid_identifier(
										ctx,
										original_query,
										loc.clone(),
										value.as_str(),
									);
									gen_identifier_or_param(
										original_query,
										value.as_str(),
										false,
										true,
									)
								}
								v => {
									generate_error!(
										ctx,
										original_query,
										add.loc.clone(),
										E206,
										&v.to_string()
									);
									GeneratedValue::Unknown
								}
							},
						)
					})
					.collect::<HashMap<String, GeneratedValue>>();

				for (field_name, default_value) in default_properties.clone() {
					if !properties.contains_key(field_name.as_str()) {
						properties.insert(field_name, default_value);
					}
				}

				properties
			}
			None => default_properties.into_iter().fold(
				HashMap::new(),
				|mut acc, (field_name, default_value)| {
					acc.insert(field_name, default_value);
					acc
				},
			),
		};
		if let Some(vec_data) = &add.data {
			let vec = match vec_data {
				VectorData::Vector(v) => {
					VecData::Standard(GeneratedValue::Literal(GenRef::Ref(format!(
						"[{}]",
						v.iter()
							.map(|f| f.to_string())
							.collect::<Vec<String>>()
							.join(",")
					))))
				}
				VectorData::Identifier(i) => {
					is_valid_identifier(ctx, original_query, add.loc.clone(), i.as_str());
					if let Some(var_info) = scope.get(i.as_str()) {
						let expected_type = Type::Array(Box::new(Type::Scalar(FieldType::F64)));
						if var_info.ty != expected_type {
							generate_error!(
								ctx,
								original_query,
								add.loc.clone(),
								E205,
								i.as_str(),
								&var_info.ty.to_string(),
								"[F64]",
								"AddV",
								ty.as_str()
							);
						}
					} else {
						generate_error!(ctx, original_query, add.loc.clone(), E301, i.as_str());
					}
					let id = gen_identifier_or_param(original_query, i.as_str(), true, false);
					VecData::Standard(id)
				}
				VectorData::Embed(e) => {
					let embed_data = match &e.value {
						EvaluatesToString::Identifier(i) => {
							type_in_scope(ctx, original_query, add.loc.clone(), scope, i.as_str());
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
			};
			let add_v = AddV {
				vec,
				label,
				properties: Some(properties.into_iter().collect()),
			};
			let stmt = GeneratedStatement::Traversal(GeneratedTraversal {
				source_step: Separator::Period(SourceStep::AddV(add_v)),
				steps: vec![],
				traversal_type: TraversalType::Mut,
				should_collect: ShouldCollect::ToObj,
				..Default::default()
			});
			gen_query.is_mut = true;
			return (Type::Vector(Some(ty.to_string())), Some(stmt));
		}
	}
	generate_error!(
		ctx,
		original_query,
		add.loc.clone(),
		E304,
		["vector"],
		["vector"]
	);
	(Type::Vector(None), None)
}

pub(crate) fn handle_search_vector<'a>(
	ctx: &mut Ctx<'a>,
	sv: &'a SearchVector,
	scope: &mut HashMap<&'a str, VariableInfo>,
	original_query: &'a Query,
	gen_query: &mut GeneratedQuery,
	_parent_ty: Option<Type>,
) -> (Type, Option<GeneratedStatement>) {
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
			if let Some(var_type) =
				type_in_scope(ctx, original_query, sv.loc.clone(), scope, i.as_str())
			{
				let expected_type = Type::Array(Box::new(Type::Scalar(FieldType::F64)));
				if var_type != expected_type {
					generate_error!(
						ctx,
						original_query,
						sv.loc.clone(),
						E205,
						i.as_str(),
						&var_type.to_string(),
						"[F64]",
						"SearchV",
						sv.vector_type.as_deref().unwrap_or("unknown")
					);
				}
			}
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
					type_in_scope(ctx, original_query, sv.loc.clone(), scope, i.as_str());
					EmbedData {
						data: gen_identifier_or_param(original_query, i.as_str(), true, false),
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
			EvaluatesToNumberType::I8(i) => GeneratedValue::Primitive(GenRef::Std(i.to_string())),
			EvaluatesToNumberType::I16(i) => GeneratedValue::Primitive(GenRef::Std(i.to_string())),
			EvaluatesToNumberType::I32(i) => GeneratedValue::Primitive(GenRef::Std(i.to_string())),
			EvaluatesToNumberType::I64(i) => GeneratedValue::Primitive(GenRef::Std(i.to_string())),

			EvaluatesToNumberType::U8(i) => GeneratedValue::Primitive(GenRef::Std(i.to_string())),
			EvaluatesToNumberType::U16(i) => GeneratedValue::Primitive(GenRef::Std(i.to_string())),
			EvaluatesToNumberType::U32(i) => GeneratedValue::Primitive(GenRef::Std(i.to_string())),
			EvaluatesToNumberType::U64(i) => GeneratedValue::Primitive(GenRef::Std(i.to_string())),
			EvaluatesToNumberType::U128(i) => GeneratedValue::Primitive(GenRef::Std(i.to_string())),
			EvaluatesToNumberType::Identifier(i) => {
				is_valid_identifier(ctx, original_query, sv.loc.clone(), i.as_str());
				type_in_scope(ctx, original_query, sv.loc.clone(), scope, i.as_str());
				gen_identifier_or_param(original_query, i, false, false)
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

	let pre_filter: Option<Vec<BoExp>> = match &sv.pre_filter {
		Some(expr) => {
			let (_, stmt) = crate::helixc::analyzer::methods::infer_expr_type::infer_expr_type(
				ctx,
				expr,
				scope,
				original_query,
				Some(Type::Vector(sv.vector_type.clone())),
				gen_query,
			);
			if stmt.is_none() {
				return (Type::Vector(sv.vector_type.clone()), None);
			}
			let stmt = stmt.unwrap();
			let mut gen_traversal = GeneratedTraversal {
				traversal_type: TraversalType::FromIter(GenRef::Std("v".to_string())),
				steps: vec![],
				should_collect: ShouldCollect::ToVec,
				source_step: Separator::Empty(SourceStep::Anonymous),
				..Default::default()
			};
			match stmt {
				GeneratedStatement::Traversal(tr) => {
					gen_traversal
						.steps
						.push(Separator::Period(GeneratedStep::Where(Where::Ref(
							WhereRef {
								expr: BoExp::Expr(tr),
							},
						))));
				}
				GeneratedStatement::BoExp(expr) => {
					gen_traversal
						.steps
						.push(Separator::Period(GeneratedStep::Where(match expr {
							BoExp::Exists(mut traversal) => {
								traversal.should_collect = ShouldCollect::No;
								Where::Ref(WhereRef {
									expr: BoExp::Exists(traversal),
								})
							}
							_ => Where::Ref(WhereRef { expr }),
						})));
				}
				_ => {
					return (Type::Vector(sv.vector_type.clone()), None);
				}
			}
			Some(vec![BoExp::Expr(gen_traversal)])
		}
		None => None,
	};

	(
		Type::Vectors(sv.vector_type.clone()),
		Some(GeneratedStatement::Traversal(GeneratedTraversal {
			traversal_type: TraversalType::Ref,
			steps: vec![],
			should_collect: ShouldCollect::ToVec,
			source_step: Separator::Period(SourceStep::SearchVector(GeneratedSearchVector {
				label: GenRef::Literal(sv.vector_type.clone().unwrap_or_default()),
				vec,
				k,
				pre_filter,
			})),
			..Default::default()
		})),
	)
}
