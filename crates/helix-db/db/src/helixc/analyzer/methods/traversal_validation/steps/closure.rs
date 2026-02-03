use std::collections::HashMap;

use paste::paste;

use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::*;
use crate::helixc::analyzer::errors::push_query_err;
use crate::helixc::analyzer::methods::object_validation::validate_object;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::VariableInfo;
use crate::helixc::generator::queries::Query as GeneratedQuery;
use crate::helixc::generator::source_steps::SourceStep;
use crate::helixc::generator::traversal_steps::{
	ShouldCollect, Traversal as GeneratedTraversal, TraversalType,
};
use crate::helixc::generator::utils::Separator;
use crate::helixc::parser::types::*;

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
