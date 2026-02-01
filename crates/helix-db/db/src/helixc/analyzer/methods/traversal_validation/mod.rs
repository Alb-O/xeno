use std::collections::HashMap;

use paste::paste;

pub(crate) use self::start_node::*;
pub(crate) use self::steps::*;
use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::methods::exclude_validation::validate_exclude;
use crate::helixc::analyzer::methods::graph_step_validation::apply_graph_step;
use crate::helixc::analyzer::methods::infer_expr_type::infer_expr_type;
use crate::helixc::analyzer::methods::object_validation::validate_object;
use crate::helixc::analyzer::types::{AggregateInfo, Type};
use crate::helixc::analyzer::utils::{
	VariableInfo, gen_identifier_or_param, is_valid_identifier, type_in_scope,
};
use crate::helixc::generator::bool_ops::BoExp;
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::traversal_steps::{
	AggregateBy, GroupBy, ShouldCollect, Step as GeneratedStep, Traversal as GeneratedTraversal,
	TraversalType, Where, WhereRef,
};
use crate::helixc::generator::utils::{GenRef, Separator};
use crate::helixc::parser::types::*;

mod start_node;
mod steps;
mod utils;

#[cfg(test)]
mod tests;

pub(crate) fn validate_traversal<'a>(
	ctx: &mut Ctx<'a>,
	tr: &'a Traversal,
	scope: &mut HashMap<&'a str, VariableInfo>,
	original_query: &'a Query,
	parent_ty: Option<Type>,
	gen_traversal: &mut GeneratedTraversal,
	gen_query: &mut GeneratedQuery,
) -> Option<Type> {
	let mut previous_step = None;
	let mut cur_ty = validate_start_node(
		ctx,
		tr,
		scope,
		original_query,
		parent_ty.clone(),
		gen_traversal,
		gen_query,
	)?;

	// Track excluded fields for property validation
	let mut excluded = HashMap::new();

	// Stream through the steps
	let number_of_steps = match tr.steps.len() {
		0 => 0,
		n => n - 1,
	};

	for (i, graph_step) in tr.steps.iter().enumerate() {
		let step = &graph_step.step;
		match step {
			StepType::Node(gs) | StepType::Edge(gs) => {
				match apply_graph_step(
					ctx,
					gs,
					&cur_ty,
					original_query,
					gen_traversal,
					scope,
					gen_query,
				) {
					Some(new_ty) => {
						cur_ty = new_ty;
					}
					None => { /* error already recorded */ }
				}
				excluded.clear(); // Traversal to a new element resets exclusions
			}
			StepType::First => {
				cur_ty = cur_ty.clone().into_single();
				excluded.clear();
				gen_traversal.should_collect = ShouldCollect::ToObj;
			}

			StepType::Count => {
				cur_ty = Type::Count;
				excluded.clear();
				gen_traversal
					.steps
					.push(Separator::Period(GeneratedStep::Count));
				gen_traversal.should_collect = ShouldCollect::No;
			}

			StepType::Exclude(ex) => {
				// checks if exclude is either the last step or the step before an object remapping or closure
				if !(i == number_of_steps
					|| (i != number_of_steps - 1
						&& (!matches!(tr.steps[i + 1].step, StepType::Closure(_))
							|| !matches!(tr.steps[i + 1].step, StepType::Object(_)))))
				{
					generate_error!(ctx, original_query, ex.loc.clone(), E644);
				}
				validate_exclude(ctx, &cur_ty, tr, ex, &excluded, original_query);
				for (_, key) in &ex.fields {
					excluded.insert(key.as_str(), ex.loc.clone());
					gen_traversal.excluded_fields.push(key.clone());
				}
			}

			StepType::Object(obj) => {
				let mut fields_out = vec![];
				cur_ty = validate_object(
					ctx,
					&cur_ty,
					obj,
					original_query,
					gen_traversal,
					&mut fields_out,
					scope,
					gen_query,
				)
				.ok()?;
			}

			StepType::Where(expr) => {
				let (_, stmt) = infer_expr_type(
					ctx,
					expr,
					scope,
					original_query,
					Some(cur_ty.clone()),
					gen_query,
				);
				if let Some(stmt) = stmt {
					match stmt {
						crate::helixc::generator::statements::Statement::Traversal(tr) => {
							gen_traversal
								.steps
								.push(Separator::Period(GeneratedStep::Where(Where::Ref(
									WhereRef {
										expr: BoExp::Expr(tr),
									},
								))));
						}
						crate::helixc::generator::statements::Statement::BoExp(expr) => {
							let where_expr = match expr {
								BoExp::Not(inner_expr) => {
									if let BoExp::Exists(mut traversal) = *inner_expr {
										traversal.should_collect = ShouldCollect::No;
										Where::Ref(WhereRef {
											expr: BoExp::Not(Box::new(BoExp::Exists(traversal))),
										})
									} else {
										Where::Ref(WhereRef {
											expr: BoExp::Not(inner_expr),
										})
									}
								}
								BoExp::Exists(mut traversal) => {
									traversal.should_collect = ShouldCollect::No;
									Where::Ref(WhereRef {
										expr: BoExp::Exists(traversal),
									})
								}
								_ => Where::Ref(WhereRef { expr }),
							};
							gen_traversal
								.steps
								.push(Separator::Period(GeneratedStep::Where(where_expr)));
						}
						_ => {
							generate_error!(
								ctx,
								original_query,
								expr.loc.clone(),
								E655,
								"unexpected statement type in Where clause"
							);
						}
					}
				}
			}
			StepType::BooleanOperation(b_op) => {
				cur_ty = validate_boolean_operation(
					ctx,
					b_op,
					&previous_step,
					&cur_ty,
					original_query,
					scope,
					gen_query,
					gen_traversal,
					parent_ty.clone(),
				)?;
			}
			StepType::Aggregate(aggr) => {
				let properties = aggr
					.properties
					.iter()
					.map(|p| GenRef::Std(format!("\"{}\".to_string()", p.clone())))
					.collect::<Vec<_>>();
				let should_count = matches!(previous_step, Some(StepType::Count));
				let _ = gen_traversal.steps.pop();

				let property_names = aggr.properties.clone();
				cur_ty = Type::Aggregate(AggregateInfo {
					source_type: Box::new(cur_ty.clone()),
					properties: property_names,
					is_count: should_count,
					is_group_by: false,
				});

				gen_traversal.should_collect = ShouldCollect::Try;
				gen_traversal
					.steps
					.push(Separator::Period(GeneratedStep::AggregateBy(AggregateBy {
						properties,
						should_count,
					})))
			}
			StepType::GroupBy(gb) => {
				let properties = gb
					.properties
					.iter()
					.map(|p| GenRef::Std(format!("\"{}\".to_string()", p.clone())))
					.collect::<Vec<_>>();
				let should_count = matches!(previous_step, Some(StepType::Count));
				let _ = gen_traversal.steps.pop();

				let property_names = gb.properties.clone();
				cur_ty = Type::Aggregate(AggregateInfo {
					source_type: Box::new(cur_ty.clone()),
					properties: property_names,
					is_count: should_count,
					is_group_by: true,
				});

				gen_traversal.should_collect = ShouldCollect::Try;
				gen_traversal
					.steps
					.push(Separator::Period(GeneratedStep::GroupBy(GroupBy {
						properties,
						should_count,
					})))
			}
			StepType::Update(update) => {
				cur_ty = validate_update_step(
					ctx,
					update,
					&cur_ty,
					original_query,
					scope,
					gen_traversal,
				)?;
				excluded.clear();
			}

			StepType::Upsert(upsert) => {
				cur_ty = validate_upsert_step(
					ctx,
					upsert,
					&cur_ty,
					original_query,
					scope,
					gen_traversal,
				)?;
				excluded.clear();
			}

			StepType::UpsertN(upsert) => {
				cur_ty = validate_upsert_n_step(
					ctx,
					upsert,
					&cur_ty,
					original_query,
					scope,
					gen_traversal,
					gen_query,
				)?;
				excluded.clear();
			}

			StepType::UpsertE(upsert) => {
				cur_ty = validate_upsert_e_step(
					ctx,
					upsert,
					&cur_ty,
					original_query,
					scope,
					gen_traversal,
					gen_query,
				)?;
				excluded.clear();
			}

			StepType::UpsertV(upsert) => {
				cur_ty = validate_upsert_v_step(
					ctx,
					upsert,
					&cur_ty,
					original_query,
					scope,
					gen_traversal,
					gen_query,
				)?;
				excluded.clear();
			}

			StepType::AddEdge(add) => {
				if let Some(ref ty) = add.edge_type
					&& !ctx.edge_map.contains_key(ty.as_str())
				{
					generate_error!(ctx, original_query, add.loc.clone(), E102, ty);
				}
				cur_ty = Type::Edges(add.edge_type.clone());
				excluded.clear();
			}

			StepType::Range(range) => {
				cur_ty = validate_range_step(
					ctx,
					range.as_ref(),
					&cur_ty,
					original_query,
					scope,
					gen_traversal,
				)?;
			}
			StepType::OrderBy(order_by) => {
				cur_ty = validate_order_by_step(
					ctx,
					order_by,
					&cur_ty,
					original_query,
					scope,
					gen_traversal,
					gen_query,
				)?;
			}
			StepType::Closure(cl) => {
				cur_ty = validate_closure_step(
					ctx,
					cl,
					&cur_ty,
					original_query,
					scope,
					gen_traversal,
					gen_query,
					i,
					number_of_steps,
				)?;
			}
			StepType::RerankRRF(rerank_rrf) => {
				let k = rerank_rrf.k.as_ref().map(|k_expr| match &k_expr.expr {
					ExpressionType::Identifier(id) => {
						let _ = is_valid_identifier(
							ctx,
							original_query,
							k_expr.loc.clone(),
							id.as_str(),
						);
						let _ = type_in_scope(
							ctx,
							original_query,
							k_expr.loc.clone(),
							scope,
							id.as_str(),
						);
						gen_identifier_or_param(original_query, id.as_str(), false, true)
					}
					ExpressionType::IntegerLiteral(val) => {
						crate::helixc::generator::utils::GeneratedValue::Primitive(
							crate::helixc::generator::utils::GenRef::Std(val.to_string()),
						)
					}
					ExpressionType::FloatLiteral(val) => {
						crate::helixc::generator::utils::GeneratedValue::Primitive(
							crate::helixc::generator::utils::GenRef::Std(val.to_string()),
						)
					}
					_ => {
						generate_error!(
							ctx,
							original_query,
							k_expr.loc.clone(),
							E206,
							&k_expr.expr.to_string()
						);
						crate::helixc::generator::utils::GeneratedValue::Unknown
					}
				});

				gen_traversal
					.steps
					.push(Separator::Period(GeneratedStep::RerankRRF(
						crate::helixc::generator::traversal_steps::RerankRRF { k },
					)));
			}
			StepType::RerankMMR(rerank_mmr) => {
				let lambda = match &rerank_mmr.lambda.expr {
					ExpressionType::Identifier(id) => {
						let _ = is_valid_identifier(
							ctx,
							original_query,
							rerank_mmr.lambda.loc.clone(),
							id.as_str(),
						);
						let _ = type_in_scope(
							ctx,
							original_query,
							rerank_mmr.lambda.loc.clone(),
							scope,
							id.as_str(),
						);
						Some(gen_identifier_or_param(
							original_query,
							id.as_str(),
							false,
							true,
						))
					}
					ExpressionType::FloatLiteral(val) => {
						Some(crate::helixc::generator::utils::GeneratedValue::Primitive(
							crate::helixc::generator::utils::GenRef::Std(val.to_string()),
						))
					}
					ExpressionType::IntegerLiteral(val) => {
						Some(crate::helixc::generator::utils::GeneratedValue::Primitive(
							crate::helixc::generator::utils::GenRef::Std(val.to_string()),
						))
					}
					_ => {
						generate_error!(
							ctx,
							original_query,
							rerank_mmr.lambda.loc.clone(),
							E206,
							&rerank_mmr.lambda.expr.to_string()
						);
						None
					}
				};

				let distance = if let Some(MMRDistance::Identifier(id)) = &rerank_mmr.distance {
					let _ = is_valid_identifier(
						ctx,
						original_query,
						rerank_mmr.loc.clone(),
						id.as_str(),
					);
					let _ = type_in_scope(
						ctx,
						original_query,
						rerank_mmr.loc.clone(),
						scope,
						id.as_str(),
					);
					Some(
						crate::helixc::generator::traversal_steps::MMRDistanceMethod::Identifier(
							id.clone(),
						),
					)
				} else {
					rerank_mmr.distance.as_ref().map(|d| match d {
						MMRDistance::Cosine => {
							crate::helixc::generator::traversal_steps::MMRDistanceMethod::Cosine
						}
						MMRDistance::Euclidean => {
							crate::helixc::generator::traversal_steps::MMRDistanceMethod::Euclidean
						}
						MMRDistance::DotProduct => {
							crate::helixc::generator::traversal_steps::MMRDistanceMethod::DotProduct
						}
						MMRDistance::Identifier(id) => {
							crate::helixc::generator::traversal_steps::MMRDistanceMethod::Identifier(
								id.clone(),
							)
						}
					})
				};

				gen_traversal
					.steps
					.push(Separator::Period(GeneratedStep::RerankMMR(
						crate::helixc::generator::traversal_steps::RerankMMR { lambda, distance },
					)));
			}
		}
		previous_step = Some(step.clone());
	}
	match gen_traversal.traversal_type {
		TraversalType::Mut | TraversalType::Update(_) | TraversalType::Upsert { .. } => {
			gen_query.is_mut = true;
		}
		_ => {}
	}
	Some(cur_ty)
}
