//! Semantic analyzer for Helix‑QL.
use std::collections::HashMap;

use paste::paste;

pub(crate) use self::handlers::*;
use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::methods::traversal_validation::validate_traversal;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::{
	DEFAULT_VAR_NAME, VariableInfo, gen_identifier_or_param, is_in_scope, is_valid_identifier,
	type_in_scope,
};
use crate::helixc::generator::bool_ops::BoExp;
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::source_steps::{SearchBM25, SourceStep};
use crate::helixc::generator::statements::Statement as GeneratedStatement;
use crate::helixc::generator::traversal_steps::{
	ShouldCollect, Traversal as GeneratedTraversal, TraversalType,
};
use crate::helixc::generator::utils::{GenRef, GeneratedValue, Separator};
use crate::helixc::parser::types::*;

mod handlers;

#[cfg(test)]
mod tests;

pub(crate) fn infer_expr_type<'a>(
	ctx: &mut Ctx<'a>,
	expression: &'a Expression,
	scope: &mut HashMap<&'a str, VariableInfo>,
	original_query: &'a Query,
	parent_ty: Option<Type>,
	gen_query: &mut GeneratedQuery,
) -> (Type, Option<GeneratedStatement>) {
	use ExpressionType::*;
	let expr: &ExpressionType = &expression.expr;
	match expr {
		Identifier(name) => {
			is_valid_identifier(ctx, original_query, expression.loc.clone(), name.as_str());
			match scope.get(name.as_str()) {
				Some(var_info) => (
					var_info.ty.clone(),
					Some(GeneratedStatement::Identifier(GenRef::Std(name.clone()))),
				),

				None => {
					generate_error!(
						ctx,
						original_query,
						expression.loc.clone(),
						E301,
						name.as_str()
					);
					(Type::Unknown, None)
				}
			}
		}

		IntegerLiteral(i) => (
			Type::Scalar(FieldType::I32),
			Some(GeneratedStatement::Literal(GenRef::Literal(i.to_string()))),
		),
		FloatLiteral(f) => (
			Type::Scalar(FieldType::F64),
			Some(GeneratedStatement::Literal(GenRef::Literal(f.to_string()))),
		),
		StringLiteral(s) => (
			Type::Scalar(FieldType::String),
			Some(GeneratedStatement::Literal(GenRef::Literal(s.to_string()))),
		),
		BooleanLiteral(b) => (
			Type::Boolean,
			Some(GeneratedStatement::Literal(GenRef::Literal(b.to_string()))),
		),
		ArrayLiteral(a) => {
			let mut inner_array_ty = None;
			let result = a.iter().try_fold(Vec::new(), |mut stmts, e| {
				let (ty, stmt) =
					infer_expr_type(ctx, e, scope, original_query, parent_ty.clone(), gen_query);
				let type_str = ty.kind_str();
				if let Some(inner_array_ty_val) = &inner_array_ty {
					if inner_array_ty_val != &ty {
						generate_error!(ctx, original_query, e.loc.clone(), E306, type_str);
					}
				} else {
					inner_array_ty = Some(ty);
				}
				match stmt {
					Some(s) => {
						stmts.push(s);
						Ok(stmts)
					}
					None => {
						generate_error!(ctx, original_query, e.loc.clone(), E306, type_str);
						Err(())
					}
				}
			});
			match result {
				Ok(stmts) => (
					Type::Array(Box::new(inner_array_ty.unwrap_or(Type::Unknown))),
					Some(GeneratedStatement::Array(stmts)),
				),
				Err(()) => (Type::Unknown, Some(GeneratedStatement::Empty)),
			}
		}
		Traversal(tr) => {
			let mut gen_traversal = GeneratedTraversal::default();
			let final_ty = validate_traversal(
				ctx,
				tr,
				scope,
				original_query,
				parent_ty.clone(),
				&mut gen_traversal,
				gen_query,
			);
			let stmt = GeneratedStatement::Traversal(gen_traversal);

			if matches!(expr, Exists(_)) {
				(Type::Boolean, Some(stmt))
			} else if let Some(final_ty) = final_ty {
				(final_ty, Some(stmt))
			} else {
				generate_error!(ctx, original_query, tr.loc.clone(), E601, &tr.loc.span);
				(Type::Unknown, None)
			}
		}

		AddNode(add) => handle_add_node(ctx, add, scope, original_query, gen_query),
		AddEdge(add) => handle_add_edge(ctx, add, scope, original_query, gen_query),
		AddVector(add) => handle_add_vector(ctx, add, scope, original_query, gen_query),
		SearchVector(sv) => {
			handle_search_vector(ctx, sv, scope, original_query, gen_query, parent_ty)
		}

		And(exprs) => {
			let exprs = exprs
				.iter()
				.map(|expr| {
					let (ty, stmt) = infer_expr_type(
						ctx,
						expr,
						scope,
						original_query,
						parent_ty.clone(),
						gen_query,
					);

					let Some(stmt) = stmt else {
						return BoExp::Empty;
					};
					match stmt {
						GeneratedStatement::BoExp(expr) => match expr {
							BoExp::Exists(mut traversal) => {
								traversal.should_collect = ShouldCollect::No;
								BoExp::Exists(traversal)
							}
							BoExp::Not(inner_expr) => {
								if let BoExp::Exists(mut traversal) = *inner_expr {
									traversal.should_collect = ShouldCollect::No;
									BoExp::Exists(traversal)
								} else {
									BoExp::Not(inner_expr)
								}
							}
							_ => expr,
						},
						GeneratedStatement::Traversal(tr) => BoExp::Expr(tr),
						_ => {
							generate_error!(
								ctx,
								original_query,
								expr.loc.clone(),
								E306,
								ty.kind_str()
							);
							BoExp::Empty
						}
					}
				})
				.collect::<Vec<_>>();
			(
				Type::Boolean,
				Some(GeneratedStatement::BoExp(BoExp::And(exprs))),
			)
		}
		Or(exprs) => {
			let exprs = exprs
				.iter()
				.map(|expr| {
					let (ty, stmt) = infer_expr_type(
						ctx,
						expr,
						scope,
						original_query,
						parent_ty.clone(),
						gen_query,
					);

					let Some(stmt) = stmt else {
						return BoExp::Empty;
					};
					match stmt {
						GeneratedStatement::BoExp(expr) => match expr {
							BoExp::Exists(mut traversal) => {
								traversal.should_collect = ShouldCollect::No;
								BoExp::Exists(traversal)
							}
							BoExp::Not(inner_expr) => {
								if let BoExp::Exists(mut traversal) = *inner_expr {
									traversal.should_collect = ShouldCollect::No;
									BoExp::Exists(traversal)
								} else {
									BoExp::Not(inner_expr)
								}
							}
							_ => expr,
						},
						GeneratedStatement::Traversal(tr) => BoExp::Expr(tr),
						_ => {
							generate_error!(
								ctx,
								original_query,
								expr.loc.clone(),
								E306,
								ty.kind_str()
							);
							BoExp::Empty
						}
					}
				})
				.collect::<Vec<_>>();
			(
				Type::Boolean,
				Some(GeneratedStatement::BoExp(BoExp::Or(exprs))),
			)
		}
		Not(expr) => {
			let (ty, stmt) =
				infer_expr_type(ctx, expr, scope, original_query, parent_ty, gen_query);

			let Some(stmt) = stmt else {
				return (Type::Unknown, None);
			};
			match stmt {
				GeneratedStatement::BoExp(expr) => (
					Type::Boolean,
					Some(GeneratedStatement::BoExp(BoExp::Not(Box::new(expr)))),
				),
				_ => {
					generate_error!(ctx, original_query, expr.loc.clone(), E306, ty.kind_str());
					(Type::Unknown, None)
				}
			}
		}
		Exists(expr) => {
			let (_, stmt) =
				infer_expr_type(ctx, &expr.expr, scope, original_query, parent_ty, gen_query);
			if stmt.is_none() {
				return (Type::Boolean, None);
			}
			let traversal = match stmt.unwrap() {
				GeneratedStatement::Traversal(mut tr) => {
					match tr.source_step.inner() {
						SourceStep::Identifier(id) => {
							let source_variable = id.inner().clone();
							let is_single = scope
								.get(source_variable.as_str())
								.map(|var_info| var_info.is_single)
								.unwrap_or(false);

							tr.traversal_type = if is_single {
								TraversalType::FromSingle(GenRef::Std(source_variable))
							} else {
								TraversalType::FromIter(GenRef::Std(source_variable))
							};
						}
						SourceStep::Anonymous => {
							tr.traversal_type = TraversalType::FromSingle(GenRef::Std(
								DEFAULT_VAR_NAME.to_string(),
							));
						}
						_ => {}
					}
					tr.should_collect = ShouldCollect::No;
					tr
				}
				_ => {
					return (Type::Boolean, None);
				}
			};
			(
				Type::Boolean,
				Some(GeneratedStatement::BoExp(BoExp::Exists(traversal))),
			)
		}
		MathFunctionCall(_math_call) => (Type::Scalar(FieldType::F64), None),
		Empty => (Type::Unknown, Some(GeneratedStatement::Empty)),
		BM25Search(bm25_search) => {
			if let Some(ref ty) = bm25_search.type_arg
				&& !ctx.node_set.contains(ty.as_str())
			{
				generate_error!(
					ctx,
					original_query,
					bm25_search.loc.clone(),
					E101,
					ty.as_str()
				);
			}
			let vec = match &bm25_search.data {
				Some(ValueType::Literal { value, loc: _ }) => {
					GeneratedValue::Literal(GenRef::Std(value.inner_stringify()))
				}
				Some(ValueType::Identifier { value: i, loc: _ }) => {
					is_valid_identifier(ctx, original_query, bm25_search.loc.clone(), i.as_str());

					if is_in_scope(scope, i.as_str()) {
						gen_identifier_or_param(original_query, i, true, false)
					} else {
						generate_error!(
							ctx,
							original_query,
							bm25_search.loc.clone(),
							E301,
							i.as_str()
						);
						GeneratedValue::Unknown
					}
				}
				_ => {
					generate_error!(
						ctx,
						original_query,
						bm25_search.loc.clone(),
						E305,
						["vector_data", "SearchV"],
						["vector_data"]
					);
					GeneratedValue::Unknown
				}
			};
			let k = match &bm25_search.k {
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
						is_valid_identifier(
							ctx,
							original_query,
							bm25_search.loc.clone(),
							i.as_str(),
						);
						type_in_scope(
							ctx,
							original_query,
							bm25_search.loc.clone(),
							scope,
							i.as_str(),
						);
						gen_identifier_or_param(original_query, i, false, false)
					}
					_ => {
						generate_error!(
							ctx,
							original_query,
							bm25_search.loc.clone(),
							E305,
							["k", "SearchBM25"],
							["k"]
						);
						GeneratedValue::Unknown
					}
				},
				None => {
					generate_error!(
						ctx,
						original_query,
						bm25_search.loc.clone(),
						E601,
						&bm25_search.loc.span
					);
					GeneratedValue::Unknown
				}
			};

			let search_bm25 = SearchBM25 {
				type_arg: GenRef::Literal(bm25_search.type_arg.clone().unwrap_or_default()),
				query: vec,
				k,
			};
			(
				Type::Nodes(bm25_search.type_arg.clone()),
				Some(GeneratedStatement::Traversal(GeneratedTraversal {
					traversal_type: TraversalType::Ref,
					steps: vec![],
					should_collect: ShouldCollect::ToVec,
					source_step: Separator::Period(SourceStep::SearchBM25(search_bm25)),
					..Default::default()
				})),
			)
		}
	}
}
