//! Semantic analyzer for Helix‑QL.
use std::collections::HashMap;

use paste::paste;

pub(crate) use self::analyze_return::*;
use crate::generate_error;
use crate::helixc::analyzer::Ctx;
use crate::helixc::analyzer::error_codes::ErrorCode;
use crate::helixc::analyzer::errors::{push_query_err, push_query_warn};
use crate::helixc::analyzer::methods::statement_validation::validate_statements;
use crate::helixc::analyzer::types::Type;
use crate::helixc::analyzer::utils::{VariableInfo, is_valid_identifier};
use crate::helixc::generator::queries::{Parameter as GeneratedParameter, Query as GeneratedQuery};
use crate::helixc::parser::location::Loc;
use crate::helixc::parser::types::*;

mod analyze_return;
mod return_fields;
mod utils;

pub(crate) fn validate_query<'a>(ctx: &mut Ctx<'a>, original_query: &'a Query) {
	let mut query = GeneratedQuery {
		name: original_query.name.clone(),
		..Default::default()
	};

	if let Some(BuiltInMacro::Model(model_name)) = &original_query.built_in_macro {
		query.embedding_model_to_use = Some(model_name.clone());
	}

	for param in &original_query.parameters {
		if let FieldType::Identifier(ref id) = param.param_type.1
			&& is_valid_identifier(ctx, original_query, param.param_type.0.clone(), id.as_str())
			&& !ctx.node_set.contains(id.as_str())
			&& !ctx.edge_map.contains_key(id.as_str())
			&& !ctx.vector_set.contains(id.as_str())
		{
			generate_error!(
				ctx,
				original_query,
				param.param_type.0.clone(),
				E209,
				&id,
				&param.name.1
			);
		}
		GeneratedParameter::unwrap_param(
			&original_query.name,
			param.clone(),
			&mut query.parameters,
			&mut query.sub_parameters,
		);
	}

	let mut scope: HashMap<&str, VariableInfo> = HashMap::new();
	for param in &original_query.parameters {
		let param_type = Type::from(param.param_type.1.clone());
		let is_single = !matches!(
			param_type,
			Type::Nodes(_) | Type::Edges(_) | Type::Vectors(_)
		);
		scope.insert(
			param.name.1.as_str(),
			VariableInfo::new(param_type, is_single),
		);
	}
	for stmt in &original_query.statements {
		let statement = validate_statements(ctx, &mut scope, original_query, &mut query, stmt);
		if let Some(s) = statement {
			query.statements.push(s);
		} else {
			return;
		}
	}

	if original_query.return_values.is_empty() {
		let end = original_query.loc.end;
		push_query_warn(
			ctx,
			original_query,
			Loc::new(
				original_query.loc.filepath.clone(),
				end,
				end,
				original_query.loc.span.clone(),
			),
			ErrorCode::W101,
			"query has no RETURN clause".to_string(),
			"add `RETURN <expr>` at the end",
			None,
		);
	}
	for ret in &original_query.return_values {
		analyze_return_expr(ctx, original_query, &mut scope, &mut query, ret);
	}

	if let Some(BuiltInMacro::MCP) = &original_query.built_in_macro {
		if query.return_values.len() != 1 {
			generate_error!(
				ctx,
				original_query,
				original_query.loc.clone(),
				E401,
				&query.return_values.len().to_string()
			);
		} else {
			let return_name = query.return_values.first().unwrap().0.clone();
			query.mcp_handler = Some(return_name);
		}
	}

	ctx.output.queries.push(query);
}
