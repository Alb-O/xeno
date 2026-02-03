use std::collections::HashMap;

use paste::paste;

use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::methods::infer_expr_type::infer_expr_type;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::{
	VariableInfo, gen_identifier_or_param, is_valid_identifier, type_in_scope,
};
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::traversal_steps::{
	OrderBy as GeneratedOrderBy, Range as GeneratedRange, ShouldCollect, Step as GeneratedStep,
	Traversal as GeneratedTraversal,
};
use crate::helixc::generator::utils::{GenRef, GeneratedValue, Order, Separator};
use crate::helixc::parser::types::*;

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
