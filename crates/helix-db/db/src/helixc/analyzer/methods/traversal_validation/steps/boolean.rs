use std::collections::HashMap;

use paste::paste;

use super::super::utils::{get_reserved_property_type, is_simple_property_traversal};
use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::methods::infer_expr_type::infer_expr_type;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::{
	VariableInfo, gen_identifier_or_param, is_valid_identifier, type_in_scope,
};
use crate::helixc::generator::bool_ops::{
	BoolOp, Contains, Eq, Gt, Gte, IsIn, Lt, Lte, Neq, PropertyEq, PropertyNeq,
};
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::traversal_steps::{
	ShouldCollect, Step as GeneratedStep, Traversal as GeneratedTraversal,
};
use crate::helixc::generator::utils::{GenRef, GeneratedValue, Separator};
use crate::helixc::parser::types::*;

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
