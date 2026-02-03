use std::collections::HashMap;

use paste::paste;

use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::{
	VariableInfo, field_exists_on_item_type, gen_identifier_or_param, get_singular_type,
	is_valid_identifier, type_in_scope,
};
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::traversal_steps::{Traversal as GeneratedTraversal, TraversalType};
use crate::helixc::generator::utils::{EmbedData, GenRef, GeneratedValue, VecData};
use crate::helixc::parser::types::*;
use crate::protocol::value::Value;

pub(crate) fn validate_update_step<'a>(
	ctx: &mut Ctx<'a>,
	update: &'a Update,
	cur_ty: &Type,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	gen_traversal: &mut GeneratedTraversal,
) -> Option<Type> {
	match &cur_ty {
		Type::Node(Some(_)) | Type::Nodes(Some(_)) | Type::Edge(Some(_)) | Type::Edges(Some(_)) => {
			field_exists_on_item_type(
				ctx,
				original_query,
				get_singular_type(cur_ty.clone()),
				update
					.fields
					.iter()
					.map(|field| (field.key.as_str(), &field.loc))
					.collect(),
			);
		}
		other => {
			generate_error!(
				ctx,
				original_query,
				update.loc.clone(),
				E604,
				&other.get_type_name()
			);
			return Some(cur_ty.clone());
		}
	}
	gen_traversal.traversal_type = TraversalType::Update(Some(
		update
			.fields
			.iter()
			.map(|field| {
				(
					field.key.clone(),
					match &field.value.value {
						FieldValueType::Identifier(i) => {
							is_valid_identifier(
								ctx,
								original_query,
								field.value.loc.clone(),
								i.as_str(),
							);
							type_in_scope(
								ctx,
								original_query,
								field.value.loc.clone(),
								scope,
								i.as_str(),
							);
							gen_identifier_or_param(original_query, i.as_str(), true, true)
						}
						FieldValueType::Literal(l) => match l {
							Value::String(s) => GeneratedValue::Literal(GenRef::Literal(s.clone())),
							other => {
								GeneratedValue::Primitive(GenRef::Std(other.inner_stringify()))
							}
						},
						FieldValueType::Expression(e) => match &e.expr {
							ExpressionType::Identifier(i) => {
								is_valid_identifier(ctx, original_query, e.loc.clone(), i.as_str());
								type_in_scope(
									ctx,
									original_query,
									e.loc.clone(),
									scope,
									i.as_str(),
								);
								gen_identifier_or_param(original_query, i.as_str(), true, true)
							}
							ExpressionType::StringLiteral(i) => {
								GeneratedValue::Literal(GenRef::Literal(i.to_string()))
							}

							ExpressionType::IntegerLiteral(i) => {
								GeneratedValue::Primitive(GenRef::Std(i.to_string()))
							}
							ExpressionType::FloatLiteral(i) => {
								GeneratedValue::Primitive(GenRef::Std(i.to_string()))
							}
							ExpressionType::BooleanLiteral(i) => {
								GeneratedValue::Primitive(GenRef::Std(i.to_string()))
							}
							other => {
								generate_error!(
									ctx,
									original_query,
									e.loc.clone(),
									E206,
									&format!("{:?}", other)
								);
								GeneratedValue::Unknown
							}
						},
						other => {
							generate_error!(
								ctx,
								original_query,
								field.value.loc.clone(),
								E206,
								&format!("{:?}", other)
							);
							GeneratedValue::Unknown
						}
					},
				)
			})
			.collect(),
	));
	Some(cur_ty.clone().into_single())
}

pub(crate) fn validate_upsert_step<'a>(
	ctx: &mut Ctx<'a>,
	upsert: &'a Upsert,
	cur_ty: &Type,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	gen_traversal: &mut GeneratedTraversal,
) -> Option<Type> {
	let source = match &gen_traversal.traversal_type {
		TraversalType::FromSingle(var) | TraversalType::FromIter(var) => Some(var.clone()),
		_ => None,
	};

	let label = match &cur_ty {
		Type::Node(Some(ty))
		| Type::Nodes(Some(ty))
		| Type::Edge(Some(ty))
		| Type::Edges(Some(ty))
		| Type::Vector(Some(ty))
		| Type::Vectors(Some(ty)) => {
			field_exists_on_item_type(
				ctx,
				original_query,
				get_singular_type(cur_ty.clone()),
				upsert
					.fields
					.iter()
					.map(|field| (field.key.as_str(), &field.loc))
					.collect(),
			);
			ty.clone()
		}
		other => {
			generate_error!(
				ctx,
				original_query,
				upsert.loc.clone(),
				E604,
				&other.get_type_name()
			);
			return Some(cur_ty.clone());
		}
	};
	gen_traversal.traversal_type = TraversalType::Upsert {
		source,
		label,
		properties: Some(
			upsert
				.fields
				.iter()
				.map(|field| {
					(
						field.key.clone(),
						match &field.value.value {
							FieldValueType::Identifier(i) => {
								is_valid_identifier(
									ctx,
									original_query,
									field.value.loc.clone(),
									i.as_str(),
								);
								type_in_scope(
									ctx,
									original_query,
									field.value.loc.clone(),
									scope,
									i.as_str(),
								);
								gen_identifier_or_param(original_query, i.as_str(), true, true)
							}
							FieldValueType::Literal(l) => match l {
								Value::String(s) => {
									GeneratedValue::Literal(GenRef::Literal(s.clone()))
								}
								other => {
									GeneratedValue::Primitive(GenRef::Std(other.inner_stringify()))
								}
							},
							FieldValueType::Expression(e) => match &e.expr {
								ExpressionType::Identifier(i) => {
									is_valid_identifier(
										ctx,
										original_query,
										e.loc.clone(),
										i.as_str(),
									);
									type_in_scope(
										ctx,
										original_query,
										e.loc.clone(),
										scope,
										i.as_str(),
									);
									gen_identifier_or_param(original_query, i.as_str(), true, true)
								}
								ExpressionType::StringLiteral(i) => {
									GeneratedValue::Literal(GenRef::Literal(i.to_string()))
								}
								ExpressionType::IntegerLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								ExpressionType::FloatLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								ExpressionType::BooleanLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								other => {
									generate_error!(
										ctx,
										original_query,
										e.loc.clone(),
										E206,
										&format!("{:?}", other)
									);
									GeneratedValue::Unknown
								}
							},
							other => {
								generate_error!(
									ctx,
									original_query,
									field.value.loc.clone(),
									E206,
									&format!("{:?}", other)
								);
								GeneratedValue::Unknown
							}
						},
					)
				})
				.collect(),
		),
	};
	Some(cur_ty.clone().into_single())
}

pub(crate) fn validate_upsert_n_step<'a>(
	ctx: &mut Ctx<'a>,
	upsert: &'a UpsertN,
	cur_ty: &Type,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	gen_traversal: &mut GeneratedTraversal,
	gen_query: &mut GeneratedQuery,
) -> Option<Type> {
	let (source, source_is_plural) = match &gen_traversal.traversal_type {
		TraversalType::FromSingle(var) => (Some(var.clone()), false),
		TraversalType::FromIter(var) => (Some(var.clone()), true),
		_ => (None, true), // Default to plural for inline traversals
	};

	let label = match &cur_ty {
		Type::Node(Some(ty)) | Type::Nodes(Some(ty)) => {
			field_exists_on_item_type(
				ctx,
				original_query,
				Type::Node(Some(ty.clone())),
				upsert
					.fields
					.iter()
					.map(|field| (field.key.as_str(), &field.loc))
					.collect(),
			);
			ty.clone()
		}
		other => {
			generate_error!(
				ctx,
				original_query,
				upsert.loc.clone(),
				E604,
				&format!(
					"UpsertN requires a Node type, found {:?}",
					other.get_type_name()
				)
			);
			return Some(cur_ty.clone());
		}
	};

	gen_query.is_mut = true;
	gen_traversal.traversal_type = TraversalType::UpsertN {
		source,
		source_is_plural,
		label,
		properties: Some(
			upsert
				.fields
				.iter()
				.map(|field| {
					(
						field.key.clone(),
						match &field.value.value {
							FieldValueType::Identifier(i) => {
								is_valid_identifier(
									ctx,
									original_query,
									field.value.loc.clone(),
									i.as_str(),
								);
								type_in_scope(
									ctx,
									original_query,
									field.value.loc.clone(),
									scope,
									i.as_str(),
								);
								gen_identifier_or_param(original_query, i.as_str(), true, true)
							}
							FieldValueType::Literal(l) => match l {
								Value::String(s) => {
									GeneratedValue::Literal(GenRef::Literal(s.clone()))
								}
								other => {
									GeneratedValue::Primitive(GenRef::Std(other.inner_stringify()))
								}
							},
							FieldValueType::Expression(e) => match &e.expr {
								ExpressionType::Identifier(i) => {
									is_valid_identifier(
										ctx,
										original_query,
										e.loc.clone(),
										i.as_str(),
									);
									type_in_scope(
										ctx,
										original_query,
										e.loc.clone(),
										scope,
										i.as_str(),
									);
									gen_identifier_or_param(original_query, i.as_str(), true, true)
								}
								ExpressionType::StringLiteral(i) => {
									GeneratedValue::Literal(GenRef::Literal(i.to_string()))
								}
								ExpressionType::IntegerLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								ExpressionType::FloatLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								ExpressionType::BooleanLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								other => {
									generate_error!(
										ctx,
										original_query,
										e.loc.clone(),
										E206,
										&format!("{:?}", other)
									);
									GeneratedValue::Unknown
								}
							},
							other => {
								generate_error!(
									ctx,
									original_query,
									field.value.loc.clone(),
									E206,
									&format!("{:?}", other)
								);
								GeneratedValue::Unknown
							}
						},
					)
				})
				.collect(),
		),
	};
	Some(cur_ty.clone().into_single())
}

pub(crate) fn validate_upsert_e_step<'a>(
	ctx: &mut Ctx<'a>,
	upsert: &'a UpsertE,
	cur_ty: &Type,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	gen_traversal: &mut GeneratedTraversal,
	gen_query: &mut GeneratedQuery,
) -> Option<Type> {
	let (source, source_is_plural) = match &gen_traversal.traversal_type {
		TraversalType::FromSingle(var) => (Some(var.clone()), false),
		TraversalType::FromIter(var) => (Some(var.clone()), true),
		_ => (None, true), // Default to plural for inline traversals
	};

	let label = match &cur_ty {
		Type::Edge(Some(ty)) | Type::Edges(Some(ty)) => {
			if !upsert.fields.is_empty() {
				field_exists_on_item_type(
					ctx,
					original_query,
					Type::Edge(Some(ty.clone())),
					upsert
						.fields
						.iter()
						.map(|field| (field.key.as_str(), &field.loc))
						.collect(),
				);
			}
			ty.clone()
		}
		other => {
			generate_error!(
				ctx,
				original_query,
				upsert.loc.clone(),
				E604,
				&format!(
					"UpsertE requires an Edge type, found {:?}",
					other.get_type_name()
				)
			);
			return Some(cur_ty.clone());
		}
	};

	let from_val = match &upsert.connection.from_id {
		Some(IdType::Identifier { value, loc }) => {
			is_valid_identifier(ctx, original_query, loc.clone(), value.as_str());
			type_in_scope(ctx, original_query, loc.clone(), scope, value.as_str());
			gen_identifier_or_param(original_query, value.as_str(), false, false)
		}
		Some(IdType::Literal { value, .. }) => {
			GeneratedValue::Literal(GenRef::Literal(value.clone()))
		}
		_ => {
			generate_error!(
				ctx,
				original_query,
				upsert.loc.clone(),
				E601,
				"Missing From() for UpsertE"
			);
			GeneratedValue::Unknown
		}
	};

	let to_val = match &upsert.connection.to_id {
		Some(IdType::Identifier { value, loc }) => {
			is_valid_identifier(ctx, original_query, loc.clone(), value.as_str());
			type_in_scope(ctx, original_query, loc.clone(), scope, value.as_str());
			gen_identifier_or_param(original_query, value.as_str(), false, false)
		}
		Some(IdType::Literal { value, .. }) => {
			GeneratedValue::Literal(GenRef::Literal(value.clone()))
		}
		_ => {
			generate_error!(
				ctx,
				original_query,
				upsert.loc.clone(),
				E601,
				"Missing To() for UpsertE"
			);
			GeneratedValue::Unknown
		}
	};

	gen_query.is_mut = true;
	gen_traversal.traversal_type = TraversalType::UpsertE {
		source,
		source_is_plural,
		label,
		properties: Some(
			upsert
				.fields
				.iter()
				.map(|field| {
					(
						field.key.clone(),
						match &field.value.value {
							FieldValueType::Identifier(i) => {
								is_valid_identifier(
									ctx,
									original_query,
									field.value.loc.clone(),
									i.as_str(),
								);
								type_in_scope(
									ctx,
									original_query,
									field.value.loc.clone(),
									scope,
									i.as_str(),
								);
								gen_identifier_or_param(original_query, i.as_str(), true, true)
							}
							FieldValueType::Literal(l) => match l {
								Value::String(s) => {
									GeneratedValue::Literal(GenRef::Literal(s.clone()))
								}
								other => {
									GeneratedValue::Primitive(GenRef::Std(other.inner_stringify()))
								}
							},
							FieldValueType::Expression(e) => match &e.expr {
								ExpressionType::Identifier(i) => {
									is_valid_identifier(
										ctx,
										original_query,
										e.loc.clone(),
										i.as_str(),
									);
									type_in_scope(
										ctx,
										original_query,
										e.loc.clone(),
										scope,
										i.as_str(),
									);
									gen_identifier_or_param(original_query, i.as_str(), true, true)
								}
								ExpressionType::StringLiteral(i) => {
									GeneratedValue::Literal(GenRef::Literal(i.to_string()))
								}
								ExpressionType::IntegerLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								ExpressionType::FloatLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								ExpressionType::BooleanLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								other => {
									generate_error!(
										ctx,
										original_query,
										e.loc.clone(),
										E206,
										&format!("{:?}", other)
									);
									GeneratedValue::Unknown
								}
							},
							other => {
								generate_error!(
									ctx,
									original_query,
									field.value.loc.clone(),
									E206,
									&format!("{:?}", other)
								);
								GeneratedValue::Unknown
							}
						},
					)
				})
				.collect(),
		),
		from: from_val,
		to: to_val,
	};
	Some(cur_ty.clone().into_single())
}

pub(crate) fn validate_upsert_v_step<'a>(
	ctx: &mut Ctx<'a>,
	upsert: &'a UpsertV,
	cur_ty: &Type,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	gen_traversal: &mut GeneratedTraversal,
	gen_query: &mut GeneratedQuery,
) -> Option<Type> {
	let (source, source_is_plural) = match &gen_traversal.traversal_type {
		TraversalType::FromSingle(var) => (Some(var.clone()), false),
		TraversalType::FromIter(var) => (Some(var.clone()), true),
		_ => (None, true),
	};

	let label = match &cur_ty {
		Type::Vector(Some(ty)) | Type::Vectors(Some(ty)) => {
			field_exists_on_item_type(
				ctx,
				original_query,
				Type::Vector(Some(ty.clone())),
				upsert
					.fields
					.iter()
					.map(|field| (field.key.as_str(), &field.loc))
					.collect(),
			);
			ty.clone()
		}
		other => {
			generate_error!(
				ctx,
				original_query,
				upsert.loc.clone(),
				E604,
				&format!(
					"UpsertV requires a Vector type, found {:?}",
					other.get_type_name()
				)
			);
			return Some(cur_ty.clone());
		}
	};

	let vec_data = match &upsert.data {
		Some(VectorData::Identifier(id)) => {
			is_valid_identifier(ctx, original_query, upsert.loc.clone(), id.as_str());
			if let Some(var_info) = scope.get(id.as_str()) {
				let expected_type = Type::Array(Box::new(Type::Scalar(FieldType::F64)));
				if var_info.ty != expected_type {
					generate_error!(
						ctx,
						original_query,
						upsert.loc.clone(),
						E205,
						id.as_str(),
						&var_info.ty.to_string(),
						"[F64]",
						"UpsertV",
						&label
					);
				}
			} else {
				generate_error!(ctx, original_query, upsert.loc.clone(), E301, id.as_str());
			}
			Some(VecData::Standard(gen_identifier_or_param(
				original_query,
				id.as_str(),
				true,
				false,
			)))
		}
		Some(VectorData::Vector(vec)) => {
			let vec_str = format!(
				"&[{}]",
				vec.iter()
					.map(|f| f.to_string())
					.collect::<Vec<_>>()
					.join(", ")
			);
			Some(VecData::Standard(GeneratedValue::Primitive(GenRef::Ref(
				vec_str,
			))))
		}
		Some(VectorData::Embed(embed)) => {
			let embed_data = match &embed.value {
				EvaluatesToString::Identifier(id) => {
					is_valid_identifier(ctx, original_query, embed.loc.clone(), id.as_str());
					type_in_scope(ctx, original_query, embed.loc.clone(), scope, id.as_str());
					EmbedData {
						data: gen_identifier_or_param(original_query, id.as_str(), true, false),
						model_name: gen_query.embedding_model_to_use.clone(),
					}
				}
				EvaluatesToString::StringLiteral(s) => EmbedData {
					data: GeneratedValue::Literal(GenRef::Ref(s.clone())),
					model_name: gen_query.embedding_model_to_use.clone(),
				},
			};
			Some(VecData::Hoisted(gen_query.add_hoisted_embed(embed_data)))
		}
		None => None,
	};

	gen_query.is_mut = true;
	gen_traversal.traversal_type = TraversalType::UpsertV {
		source,
		source_is_plural,
		label,
		properties: Some(
			upsert
				.fields
				.iter()
				.map(|field| {
					(
						field.key.clone(),
						match &field.value.value {
							FieldValueType::Identifier(i) => {
								is_valid_identifier(
									ctx,
									original_query,
									field.value.loc.clone(),
									i.as_str(),
								);
								type_in_scope(
									ctx,
									original_query,
									field.value.loc.clone(),
									scope,
									i.as_str(),
								);
								gen_identifier_or_param(original_query, i.as_str(), true, true)
							}
							FieldValueType::Literal(l) => match l {
								Value::String(s) => {
									GeneratedValue::Literal(GenRef::Literal(s.clone()))
								}
								other => {
									GeneratedValue::Primitive(GenRef::Std(other.inner_stringify()))
								}
							},
							FieldValueType::Expression(e) => match &e.expr {
								ExpressionType::Identifier(i) => {
									is_valid_identifier(
										ctx,
										original_query,
										e.loc.clone(),
										i.as_str(),
									);
									type_in_scope(
										ctx,
										original_query,
										e.loc.clone(),
										scope,
										i.as_str(),
									);
									gen_identifier_or_param(original_query, i.as_str(), true, true)
								}
								ExpressionType::StringLiteral(i) => {
									GeneratedValue::Literal(GenRef::Literal(i.to_string()))
								}
								ExpressionType::IntegerLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								ExpressionType::FloatLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								ExpressionType::BooleanLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								other => {
									generate_error!(
										ctx,
										original_query,
										e.loc.clone(),
										E206,
										&format!("{:?}", other)
									);
									GeneratedValue::Unknown
								}
							},
							other => {
								generate_error!(
									ctx,
									original_query,
									field.value.loc.clone(),
									E206,
									&format!("{:?}", other)
								);
								GeneratedValue::Unknown
							}
						},
					)
				})
				.collect(),
		),
		vec_data,
	};
	Some(cur_ty.clone().into_single())
}
