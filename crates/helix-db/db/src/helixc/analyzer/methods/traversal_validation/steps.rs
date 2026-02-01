use std::collections::HashMap;

use paste::paste;

use super::utils::*;
use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::methods::infer_expr_type::infer_expr_type;
use crate::helixc::analyzer::methods::object_validation::validate_object;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::{
	VariableInfo, field_exists_on_item_type, gen_identifier_or_param, get_singular_type,
	is_valid_identifier, type_in_scope,
};
use crate::helixc::generator::bool_ops::{
	BoolOp, Contains, Eq, Gt, Gte, IsIn, Lt, Lte, Neq, PropertyEq, PropertyNeq,
};
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::source_steps::SourceStep;
use crate::helixc::generator::traversal_steps::{
	OrderBy as GeneratedOrderBy, Range as GeneratedRange, ShouldCollect, Step as GeneratedStep,
	Traversal as GeneratedTraversal, TraversalType,
};
use crate::helixc::generator::utils::{
	EmbedData, GenRef, GeneratedValue, Order, Separator, VecData,
};
use crate::helixc::parser::types::*;
use crate::protocol::value::Value;

pub(crate) fn validate_boolean_operation<'a>(
	ctx: &mut Ctx<'a>,
	b_op: &'a BooleanOp,
	previous_step: &Option<StepType>,
	cur_ty: &Type,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	gen_query: &mut GeneratedQuery,
	gen_traversal: &mut GeneratedTraversal,
	parent_ty: Option<Type>,
) -> Option<Type> {
	let Some(step) = previous_step else {
		generate_error!(
			ctx,
			original_query,
			b_op.loc.clone(),
			E657,
			"BooleanOperation"
		);
		return Some(cur_ty.clone());
	};
	let property_type = match &b_op.op {
		BooleanOpType::LessThanOrEqual(expr)
		| BooleanOpType::LessThan(expr)
		| BooleanOpType::GreaterThanOrEqual(expr)
		| BooleanOpType::GreaterThan(expr)
		| BooleanOpType::Equal(expr)
		| BooleanOpType::NotEqual(expr)
		| BooleanOpType::Contains(expr) => {
			match infer_expr_type(
				ctx,
				expr,
				scope,
				original_query,
				Some(cur_ty.clone()),
				gen_query,
			) {
				(Type::Scalar(ft), _) => ft.clone(),
				(Type::Boolean, _) => FieldType::Boolean,
				(field_type, _) => {
					generate_error!(
						ctx,
						original_query,
						b_op.loc.clone(),
						E621,
						&b_op.loc.span,
						field_type.kind_str()
					);
					return Some(field_type);
				}
			}
		}
		BooleanOpType::IsIn(expr) => {
			// IS_IN expects an array argument
			match infer_expr_type(
				ctx,
				expr,
				scope,
				original_query,
				Some(cur_ty.clone()),
				gen_query,
			) {
				(Type::Array(boxed_ty), _) => match *boxed_ty {
					Type::Scalar(ft) => ft,
					_ => {
						generate_error!(
							ctx,
							original_query,
							b_op.loc.clone(),
							E621,
							&b_op.loc.span,
							"non-scalar array elements"
						);
						return Some(Type::Unknown);
					}
				},
				(field_type, _) => {
					generate_error!(
						ctx,
						original_query,
						b_op.loc.clone(),
						E621,
						&b_op.loc.span,
						field_type.kind_str()
					);
					return Some(field_type);
				}
			}
		}
		_ => return Some(cur_ty.clone()),
	};

	// get type of field name
	let field_name = match step {
		StepType::Object(obj) => {
			let fields = &obj.fields;
			assert!(fields.len() == 1);
			Some(fields[0].value.value.clone())
		}
		_ => None,
	};
	if let Some(FieldValueType::Identifier(field_name)) = &field_name {
		is_valid_identifier(ctx, original_query, b_op.loc.clone(), field_name.as_str());
		match &cur_ty {
			Type::Scalar(ft) => {
				if ft != &property_type {
					generate_error!(
						ctx,
						original_query,
						b_op.loc.clone(),
						E622,
						field_name,
						cur_ty.kind_str(),
						&cur_ty.get_type_name(),
						&ft.to_string(),
						&property_type.to_string()
					);
				}
			}
			Type::Nodes(Some(node_ty)) | Type::Node(Some(node_ty)) => {
				// Check if this is a reserved property first
				if let Some(reserved_type) = get_reserved_property_type(field_name.as_str(), cur_ty)
				{
					// Validate the type matches
					if let FieldType::Array(inner_type) = &property_type {
						if reserved_type != **inner_type {
							generate_error!(
								ctx,
								original_query,
								b_op.loc.clone(),
								E622,
								field_name,
								cur_ty.kind_str(),
								&cur_ty.get_type_name(),
								&reserved_type.to_string(),
								&property_type.to_string()
							);
						}
					} else if reserved_type != property_type {
						generate_error!(
							ctx,
							original_query,
							b_op.loc.clone(),
							E622,
							field_name,
							cur_ty.kind_str(),
							&cur_ty.get_type_name(),
							&reserved_type.to_string(),
							&property_type.to_string()
						);
					}
				} else {
					// Not a reserved property, check schema fields
					let field_set = ctx.node_fields.get(node_ty.as_str()).cloned();
					if let Some(field_set) = field_set {
						match field_set.get(field_name.as_str()) {
							Some(field) => {
								if let FieldType::Array(inner_type) = &property_type {
									if field.field_type != **inner_type {
										generate_error!(
											ctx,
											original_query,
											b_op.loc.clone(),
											E622,
											field_name,
											cur_ty.kind_str(),
											&cur_ty.get_type_name(),
											&field.field_type.to_string(),
											&property_type.to_string()
										);
									}
								} else if field.field_type != property_type {
									generate_error!(
										ctx,
										original_query,
										b_op.loc.clone(),
										E622,
										field_name,
										cur_ty.kind_str(),
										&cur_ty.get_type_name(),
										&field.field_type.to_string(),
										&property_type.to_string()
									);
								}
							}
							None => {
								generate_error!(
									ctx,
									original_query,
									b_op.loc.clone(),
									E202,
									field_name,
									cur_ty.kind_str(),
									node_ty
								);
							}
						}
					}
				}
			}
			Type::Edges(Some(edge_ty)) | Type::Edge(Some(edge_ty)) => {
				// Check if this is a reserved property first
				if let Some(reserved_type) = get_reserved_property_type(field_name.as_str(), cur_ty)
				{
					// Validate the type matches
					if reserved_type != property_type {
						generate_error!(
							ctx,
							original_query,
							b_op.loc.clone(),
							E622,
							field_name,
							cur_ty.kind_str(),
							&cur_ty.get_type_name(),
							&reserved_type.to_string(),
							&property_type.to_string()
						);
					}
				} else {
					// Not a reserved property, check schema fields
					let field_set = ctx.edge_fields.get(edge_ty.as_str()).cloned();
					if let Some(field_set) = field_set {
						match field_set.get(field_name.as_str()) {
							Some(field) => {
								if field.field_type != property_type {
									generate_error!(
										ctx,
										original_query,
										b_op.loc.clone(),
										E622,
										field_name,
										cur_ty.kind_str(),
										&cur_ty.get_type_name(),
										&field.field_type.to_string(),
										&property_type.to_string()
									);
								}
							}
							None => {
								generate_error!(
									ctx,
									original_query,
									b_op.loc.clone(),
									E202,
									field_name,
									cur_ty.kind_str(),
									edge_ty
								);
							}
						}
					}
				}
			}
			Type::Vectors(Some(sv)) | Type::Vector(Some(sv)) => {
				// Check if this is a reserved property first
				if let Some(reserved_type) = get_reserved_property_type(field_name.as_str(), cur_ty)
				{
					// Validate the type matches
					if reserved_type != property_type {
						generate_error!(
							ctx,
							original_query,
							b_op.loc.clone(),
							E622,
							field_name,
							cur_ty.kind_str(),
							&cur_ty.get_type_name(),
							&reserved_type.to_string(),
							&property_type.to_string()
						);
					}
				} else {
					// Not a reserved property, check schema fields
					let field_set = ctx.vector_fields.get(sv.as_str()).cloned();
					if let Some(field_set) = field_set {
						match field_set.get(field_name.as_str()) {
							Some(field) => {
								if field.field_type != property_type {
									generate_error!(
										ctx,
										original_query,
										b_op.loc.clone(),
										E622,
										field_name,
										cur_ty.kind_str(),
										&cur_ty.get_type_name(),
										&field.field_type.to_string(),
										&property_type.to_string()
									);
								}
							}
							None => {
								generate_error!(
									ctx,
									original_query,
									b_op.loc.clone(),
									E202,
									field_name,
									cur_ty.kind_str(),
									sv
								);
							}
						}
					}
				}
			}
			_ => {
				generate_error!(
					ctx,
					original_query,
					b_op.loc.clone(),
					E621,
					&b_op.loc.span,
					cur_ty.kind_str()
				);
			}
		}
	}

	let op = match &b_op.op {
		BooleanOpType::LessThanOrEqual(expr) => {
			let v = match &expr.expr {
				ExpressionType::IntegerLiteral(i) => {
					GeneratedValue::Primitive(GenRef::Std(i.to_string()))
				}
				ExpressionType::FloatLiteral(f) => {
					GeneratedValue::Primitive(GenRef::Std(f.to_string()))
				}
				ExpressionType::Identifier(i) => {
					is_valid_identifier(ctx, original_query, expr.loc.clone(), i.as_str());
					type_in_scope(ctx, original_query, expr.loc.clone(), scope, i.as_str());
					gen_identifier_or_param(original_query, i.as_str(), false, true)
				}
				other => {
					generate_error!(
						ctx,
						original_query,
						expr.loc.clone(),
						E655,
						&format!("unexpected expression type in comparison: {:?}", other)
					);
					GeneratedValue::Unknown
				}
			};
			BoolOp::Lte(Lte {
				left: GeneratedValue::Primitive(GenRef::Std("*v".to_string())),
				right: v,
			})
		}
		BooleanOpType::LessThan(expr) => {
			let v = match &expr.expr {
				ExpressionType::IntegerLiteral(i) => {
					GeneratedValue::Primitive(GenRef::Std(i.to_string()))
				}
				ExpressionType::FloatLiteral(f) => {
					GeneratedValue::Primitive(GenRef::Std(f.to_string()))
				}
				ExpressionType::Identifier(i) => {
					is_valid_identifier(ctx, original_query, expr.loc.clone(), i.as_str());
					type_in_scope(ctx, original_query, expr.loc.clone(), scope, i.as_str());
					gen_identifier_or_param(original_query, i.as_str(), false, true)
				}
				other => {
					generate_error!(
						ctx,
						original_query,
						expr.loc.clone(),
						E655,
						&format!("unexpected expression type in comparison: {:?}", other)
					);
					GeneratedValue::Unknown
				}
			};
			BoolOp::Lt(Lt {
				left: GeneratedValue::Primitive(GenRef::Std("*v".to_string())),
				right: v,
			})
		}
		BooleanOpType::GreaterThanOrEqual(expr) => {
			let v = match &expr.expr {
				ExpressionType::IntegerLiteral(i) => {
					GeneratedValue::Primitive(GenRef::Std(i.to_string()))
				}
				ExpressionType::FloatLiteral(f) => {
					GeneratedValue::Primitive(GenRef::Std(f.to_string()))
				}
				ExpressionType::Identifier(i) => {
					is_valid_identifier(ctx, original_query, expr.loc.clone(), i.as_str());
					type_in_scope(ctx, original_query, expr.loc.clone(), scope, i.as_str());
					gen_identifier_or_param(original_query, i.as_str(), false, true)
				}
				other => {
					generate_error!(
						ctx,
						original_query,
						expr.loc.clone(),
						E655,
						&format!("unexpected expression type in comparison: {:?}", other)
					);
					GeneratedValue::Unknown
				}
			};
			BoolOp::Gte(Gte {
				left: GeneratedValue::Primitive(GenRef::Std("*v".to_string())),
				right: v,
			})
		}
		BooleanOpType::GreaterThan(expr) => {
			let v = match &expr.expr {
				ExpressionType::IntegerLiteral(i) => {
					GeneratedValue::Primitive(GenRef::Std(i.to_string()))
				}
				ExpressionType::FloatLiteral(f) => {
					GeneratedValue::Primitive(GenRef::Std(f.to_string()))
				}
				ExpressionType::Identifier(i) => {
					is_valid_identifier(ctx, original_query, expr.loc.clone(), i.as_str());
					type_in_scope(ctx, original_query, expr.loc.clone(), scope, i.as_str());
					gen_identifier_or_param(original_query, i.as_str(), false, true)
				}
				other => {
					generate_error!(
						ctx,
						original_query,
						expr.loc.clone(),
						E655,
						&format!("unexpected expression type in comparison: {:?}", other)
					);
					GeneratedValue::Unknown
				}
			};
			BoolOp::Gt(Gt {
				left: GeneratedValue::Primitive(GenRef::Std("*v".to_string())),
				right: v,
			})
		}
		BooleanOpType::Equal(expr) => {
			// Check if the right-hand side is a simple property traversal
			if let ExpressionType::Traversal(traversal) = &expr.expr {
				if let Some((var, property)) = is_simple_property_traversal(traversal) {
					// Use PropertyEq for simple traversals to avoid unnecessary G::from_iter
					BoolOp::PropertyEq(PropertyEq { var, property })
				} else {
					// Complex traversal - parse normally
					let mut g_traversal = GeneratedTraversal::default();
					crate::helixc::analyzer::methods::traversal_validation::validate_traversal(
						ctx,
						traversal,
						scope,
						original_query,
						parent_ty,
						&mut g_traversal,
						gen_query,
					);
					g_traversal.should_collect = ShouldCollect::ToValue;
					let v = GeneratedValue::Traversal(Box::new(g_traversal));
					BoolOp::Eq(Eq {
						left: GeneratedValue::Primitive(GenRef::Std("*v".to_string())),
						right: v,
					})
				}
			} else {
				let v = match &expr.expr {
					ExpressionType::BooleanLiteral(b) => {
						GeneratedValue::Primitive(GenRef::Std(b.to_string()))
					}
					ExpressionType::IntegerLiteral(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					ExpressionType::FloatLiteral(f) => {
						GeneratedValue::Primitive(GenRef::Std(f.to_string()))
					}
					ExpressionType::StringLiteral(s) => {
						GeneratedValue::Primitive(GenRef::Literal(s.to_string()))
					}
					ExpressionType::Identifier(i) => {
						is_valid_identifier(ctx, original_query, expr.loc.clone(), i.as_str());
						type_in_scope(ctx, original_query, expr.loc.clone(), scope, i.as_str());
						gen_identifier_or_param(original_query, i.as_str(), false, true)
					}
					other => {
						generate_error!(
							ctx,
							original_query,
							expr.loc.clone(),
							E655,
							&format!("unexpected expression type in equality: {:?}", other)
						);
						GeneratedValue::Unknown
					}
				};
				BoolOp::Eq(Eq {
					left: GeneratedValue::Primitive(GenRef::Std("*v".to_string())),
					right: v,
				})
			}
		}
		BooleanOpType::NotEqual(expr) => {
			// Check if the right-hand side is a simple property traversal
			if let ExpressionType::Traversal(traversal) = &expr.expr {
				if let Some((var, property)) = is_simple_property_traversal(traversal) {
					// Use PropertyNeq for simple traversals to avoid unnecessary G::from_iter
					BoolOp::PropertyNeq(PropertyNeq { var, property })
				} else {
					// Complex traversal - parse normally
					let mut g_traversal = GeneratedTraversal::default();
					crate::helixc::analyzer::methods::traversal_validation::validate_traversal(
						ctx,
						traversal,
						scope,
						original_query,
						parent_ty,
						&mut g_traversal,
						gen_query,
					);
					g_traversal.should_collect = ShouldCollect::ToValue;
					let v = GeneratedValue::Traversal(Box::new(g_traversal));
					BoolOp::Neq(Neq {
						left: GeneratedValue::Primitive(GenRef::Std("*v".to_string())),
						right: v,
					})
				}
			} else {
				let v = match &expr.expr {
					ExpressionType::BooleanLiteral(b) => {
						GeneratedValue::Primitive(GenRef::Std(b.to_string()))
					}
					ExpressionType::IntegerLiteral(i) => {
						GeneratedValue::Primitive(GenRef::Std(i.to_string()))
					}
					ExpressionType::FloatLiteral(f) => {
						GeneratedValue::Primitive(GenRef::Std(f.to_string()))
					}
					ExpressionType::StringLiteral(s) => {
						GeneratedValue::Primitive(GenRef::Literal(s.to_string()))
					}
					ExpressionType::Identifier(i) => {
						is_valid_identifier(ctx, original_query, expr.loc.clone(), i.as_str());
						type_in_scope(ctx, original_query, expr.loc.clone(), scope, i.as_str());
						gen_identifier_or_param(original_query, i.as_str(), false, true)
					}
					other => {
						generate_error!(
							ctx,
							original_query,
							expr.loc.clone(),
							E655,
							&format!("unexpected expression type in inequality: {:?}", other)
						);
						GeneratedValue::Unknown
					}
				};
				BoolOp::Neq(Neq {
					left: GeneratedValue::Primitive(GenRef::Std("*v".to_string())),
					right: v,
				})
			}
		}
		BooleanOpType::Contains(expr) => {
			let v = match &expr.expr {
				ExpressionType::Identifier(i) => {
					is_valid_identifier(ctx, original_query, expr.loc.clone(), i.as_str());
					type_in_scope(ctx, original_query, expr.loc.clone(), scope, i.as_str());
					gen_identifier_or_param(original_query, i.as_str(), true, false)
				}
				ExpressionType::BooleanLiteral(b) => {
					GeneratedValue::Primitive(GenRef::Std(b.to_string()))
				}
				ExpressionType::IntegerLiteral(i) => {
					GeneratedValue::Primitive(GenRef::Std(i.to_string()))
				}
				ExpressionType::FloatLiteral(f) => {
					GeneratedValue::Primitive(GenRef::Std(f.to_string()))
				}
				ExpressionType::StringLiteral(s) => {
					GeneratedValue::Primitive(GenRef::Literal(s.to_string()))
				}
				other => {
					generate_error!(
						ctx,
						original_query,
						expr.loc.clone(),
						E655,
						&format!("unexpected expression type in contains: {:?}", other)
					);
					GeneratedValue::Unknown
				}
			};
			BoolOp::Contains(Contains { value: v })
		}
		BooleanOpType::IsIn(expr) => {
			let v = match &expr.expr {
				ExpressionType::Identifier(i) => {
					is_valid_identifier(ctx, original_query, expr.loc.clone(), i.as_str());
					type_in_scope(ctx, original_query, expr.loc.clone(), scope, i.as_str());
					gen_identifier_or_param(original_query, i.as_str(), true, false)
				}
				ExpressionType::ArrayLiteral(a) => GeneratedValue::Array(GenRef::Std(
					a.iter()
						.map(|e| {
							let v = match &e.expr {
								ExpressionType::BooleanLiteral(b) => {
									GeneratedValue::Primitive(GenRef::Std(b.to_string()))
								}
								ExpressionType::IntegerLiteral(i) => {
									GeneratedValue::Primitive(GenRef::Std(i.to_string()))
								}
								ExpressionType::FloatLiteral(f) => {
									GeneratedValue::Primitive(GenRef::Std(f.to_string()))
								}
								ExpressionType::StringLiteral(s) => {
									GeneratedValue::Primitive(GenRef::Literal(s.to_string()))
								}
								// Other expression types in arrays are not supported for IS_IN
								_ => GeneratedValue::Unknown,
							};
							v.to_string()
						})
						.collect::<Vec<_>>()
						.join(", "),
				)),
				other => {
					generate_error!(
						ctx,
						original_query,
						expr.loc.clone(),
						E655,
						&format!("unexpected expression type in IS_IN: {:?}", other)
					);
					GeneratedValue::Unknown
				}
			};
			BoolOp::IsIn(IsIn { value: v })
		}
		other => {
			// Other boolean operations should have been handled above
			generate_error!(
				ctx,
				original_query,
				b_op.loc.clone(),
				E655,
				&format!("unexpected boolean operation type: {:?}", other)
			);
			return Some(cur_ty.clone());
		}
	};
	gen_traversal
		.steps
		.push(Separator::Period(GeneratedStep::BoolOp(op)));
	gen_traversal.should_collect = ShouldCollect::No;
	Some(cur_ty.clone())
}

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

pub(crate) fn validate_range_step<'a>(
	ctx: &mut Ctx<'a>,
	range: &'a (Expression, Expression),
	cur_ty: &Type,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	gen_traversal: &mut GeneratedTraversal,
) -> Option<Type> {
	let (start, end) = range;
	let (start_val, end_val) = match (&start.expr, &end.expr) {
		(ExpressionType::Identifier(i), ExpressionType::Identifier(j)) => {
			is_valid_identifier(ctx, original_query, start.loc.clone(), i.as_str());
			is_valid_identifier(ctx, original_query, end.loc.clone(), j.as_str());

			let ty = type_in_scope(ctx, original_query, start.loc.clone(), scope, i.as_str());
			if let Some(ty) = ty
				&& !ty.is_integer()
			{
				generate_error!(
					ctx,
					original_query,
					start.loc.clone(),
					E633,
					[&start.loc.span, &ty.get_type_name()],
					[i.as_str()]
				);
				return Some(cur_ty.clone());
			};
			let ty = type_in_scope(ctx, original_query, end.loc.clone(), scope, j.as_str());
			if let Some(ty) = ty
				&& !ty.is_integer()
			{
				generate_error!(
					ctx,
					original_query,
					end.loc.clone(),
					E633,
					[&end.loc.span, &ty.get_type_name()],
					[j.as_str()]
				);
				return Some(cur_ty.clone());
			}
			(
				gen_identifier_or_param(original_query, i.as_str(), false, true),
				gen_identifier_or_param(original_query, j.as_str(), false, true),
			)
		}
		(ExpressionType::IntegerLiteral(i), ExpressionType::IntegerLiteral(j)) => (
			GeneratedValue::Primitive(GenRef::Std(i.to_string())),
			GeneratedValue::Primitive(GenRef::Std(j.to_string())),
		),
		(ExpressionType::Identifier(i), ExpressionType::IntegerLiteral(j)) => {
			is_valid_identifier(ctx, original_query, start.loc.clone(), i.as_str());

			let ty = type_in_scope(ctx, original_query, start.loc.clone(), scope, i.as_str());
			if let Some(ty) = ty
				&& !ty.is_integer()
			{
				generate_error!(
					ctx,
					original_query,
					start.loc.clone(),
					E633,
					[&start.loc.span, &ty.get_type_name()],
					[i.as_str()]
				);
				return Some(cur_ty.clone());
			}

			(
				gen_identifier_or_param(original_query, i.as_str(), false, true),
				GeneratedValue::Primitive(GenRef::Std(j.to_string())),
			)
		}
		(ExpressionType::IntegerLiteral(i), ExpressionType::Identifier(j)) => {
			is_valid_identifier(ctx, original_query, end.loc.clone(), j.as_str());
			let ty = type_in_scope(ctx, original_query, end.loc.clone(), scope, j.as_str());
			if let Some(ty) = ty
				&& !ty.is_integer()
			{
				generate_error!(
					ctx,
					original_query,
					end.loc.clone(),
					E633,
					[&end.loc.span, &ty.get_type_name()],
					[j.as_str()]
				);
				return Some(cur_ty.clone());
			}
			(
				GeneratedValue::Primitive(GenRef::Std(i.to_string())),
				gen_identifier_or_param(original_query, j.as_str(), false, true),
			)
		}
		(ExpressionType::Identifier(_) | ExpressionType::IntegerLiteral(_), other) => {
			generate_error!(
				ctx,
				original_query,
				start.loc.clone(),
				E633,
				[&start.loc.span, &format!("{}", other)],
				[&format!("{}", other)]
			);
			return Some(cur_ty.clone());
		}
		(other, ExpressionType::Identifier(_) | ExpressionType::IntegerLiteral(_)) => {
			generate_error!(
				ctx,
				original_query,
				start.loc.clone(),
				E633,
				[&start.loc.span, &format!("{}", other)],
				[&format!("{}", other)]
			);
			return Some(cur_ty.clone());
		}
		(start_expr, end_expr) => {
			generate_error!(
				ctx,
				original_query,
				start.loc.clone(),
				E633,
				[&format!("({}, {})", start_expr, end_expr), "non-integer"],
				["start and end"]
			);
			return Some(cur_ty.clone());
		}
	};
	gen_traversal
		.steps
		.push(Separator::Period(GeneratedStep::Range(GeneratedRange {
			start: start_val,
			end: end_val,
		})));
	Some(cur_ty.clone())
}

pub(crate) fn validate_order_by_step<'a>(
	ctx: &mut Ctx<'a>,
	order_by: &'a OrderBy,
	cur_ty: &Type,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	gen_traversal: &mut GeneratedTraversal,
	gen_query: &mut GeneratedQuery,
) -> Option<Type> {
	let (_, stmt) = infer_expr_type(
		ctx,
		order_by.expression.as_ref(),
		scope,
		original_query,
		Some(cur_ty.clone()),
		gen_query,
	);

	if stmt.is_none() {
		return Some(cur_ty.clone());
	}
	match stmt.unwrap() {
		crate::helixc::generator::statements::Statement::Traversal(traversal) => {
			gen_traversal
				.steps
				.push(Separator::Period(GeneratedStep::OrderBy(
					GeneratedOrderBy {
						traversal,
						order: match order_by.order_by_type {
							OrderByType::Asc => Order::Asc,
							OrderByType::Desc => Order::Desc,
						},
					},
				)));
			gen_traversal.should_collect = ShouldCollect::ToVec;
		}
		_ => {
			generate_error!(
				ctx,
				original_query,
				order_by.expression.loc.clone(),
				E655,
				"OrderBy expected traversal expression"
			);
		}
	}
	Some(cur_ty.clone())
}

pub(crate) fn validate_closure_step<'a>(
	ctx: &mut Ctx<'a>,
	cl: &'a Closure,
	cur_ty: &Type,
	original_query: &'a Query,
	scope: &mut HashMap<&'a str, VariableInfo>,
	gen_traversal: &mut GeneratedTraversal,
	gen_query: &mut GeneratedQuery,
	i: usize,
	number_of_steps: usize,
) -> Option<Type> {
	if i != number_of_steps {
		generate_error!(ctx, original_query, cl.loc.clone(), E641);
	}
	let was_collection = matches!(cur_ty, Type::Nodes(_) | Type::Edges(_) | Type::Vectors(_));
	let closure_param_type = match cur_ty.clone() {
		Type::Nodes(label) => Type::Node(label),
		Type::Edges(label) => Type::Edge(label),
		Type::Vectors(label) => Type::Vector(label),
		other => other,
	};

	let closure_source_var = match &gen_traversal.source_step {
		Separator::Empty(SourceStep::Identifier(var))
		| Separator::Period(SourceStep::Identifier(var))
		| Separator::Newline(SourceStep::Identifier(var)) => var.inner().clone(),
		_ => match &gen_traversal.traversal_type {
			TraversalType::FromSingle(var) | TraversalType::FromIter(var) => var.inner().clone(),
			_ => String::new(),
		},
	};

	scope.insert(
		cl.identifier.as_str(),
		VariableInfo::new_with_source(closure_param_type.clone(), true, closure_source_var.clone()),
	);
	let obj = &cl.object;
	let mut fields_out = vec![];
	let mut next_ty = validate_object(
		ctx,
		&closure_param_type,
		obj,
		original_query,
		gen_traversal,
		&mut fields_out,
		scope,
		gen_query,
	)
	.ok()?;

	gen_traversal.closure_param_name = Some(cl.identifier.clone());

	for (_field_name, nested_info) in gen_traversal.nested_traversals.iter_mut() {
		nested_info.closure_param_name = Some(cl.identifier.clone());
		nested_info.closure_source_var = Some(closure_source_var.clone());
	}

	if was_collection {
		gen_traversal.should_collect = ShouldCollect::ToVec;
		next_ty = match next_ty {
			Type::Node(label) => Type::Nodes(label),
			Type::Edge(label) => Type::Edges(label),
			Type::Vector(label) => Type::Vectors(label),
			other => other,
		};
	}

	scope.remove(cl.identifier.as_str());
	Some(next_ty)
}
