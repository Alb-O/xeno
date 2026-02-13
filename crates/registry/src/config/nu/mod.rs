//! Nu script configuration parsing for Xeno.

use std::collections::HashSet;
use std::path::{Component, Path};

use nu_protocol::ast::{Argument, Block, Expr, Expression, ListItem, RecordItem};
use nu_protocol::engine::{Stack, StateWorkingSet};
use nu_protocol::{BlockId, PipelineData, Span, Value};

use super::{Config, ConfigError, Result};

/// Evaluate a Nu script and parse its resulting value as [`Config`].
pub fn eval_config_str(input: &str, fname: &str) -> Result<Config> {
	let config_root = Path::new(fname).parent();
	let mut engine_state = nu_cmd_lang::create_default_context();
	if let Some(cwd) = config_root.and_then(|p| std::fs::canonicalize(p).ok()) {
		engine_state.add_env_var("PWD".to_string(), Value::string(cwd.to_string_lossy().to_string(), Span::unknown()));
	}
	let mut working_set = StateWorkingSet::new(&engine_state);
	let block = nu_parser::parse(&mut working_set, Some(fname), input.as_bytes(), false);

	if let Some(error) = working_set.parse_errors.first() {
		return Err(ConfigError::NuParse(error.to_string()));
	}
	if let Some(error) = working_set.compile_errors.first() {
		return Err(ConfigError::NuParse(error.to_string()));
	}

	ensure_working_set_is_sandboxed(&working_set, block.as_ref(), config_root)?;

	let delta = working_set.render();
	engine_state.merge_delta(delta).map_err(|error| ConfigError::NuParse(error.to_string()))?;

	let mut stack = Stack::new();
	let eval_block = nu_engine::get_eval_block(&engine_state);
	let execution = eval_block(&engine_state, &mut stack, block.as_ref(), PipelineData::empty()).map_err(|error| ConfigError::NuRuntime(error.to_string()))?;
	let value = execution
		.body
		.into_value(Span::unknown())
		.map_err(|error| ConfigError::NuRuntime(error.to_string()))?;

	if value.as_record().is_err() {
		return Err(ConfigError::NuRuntime("config.nu must evaluate to a record value".to_string()));
	}

	crate::config::nuon::parse_config_value(&value)
}

fn ensure_working_set_is_sandboxed(working_set: &StateWorkingSet<'_>, root: &Block, config_root: Option<&Path>) -> Result<()> {
	let mut visited_blocks = HashSet::new();
	check_block(working_set, root, &mut visited_blocks, config_root)?;

	let base = working_set.permanent_state.num_blocks();
	for idx in 0..working_set.delta.blocks.len() {
		let block_id = BlockId::new(base + idx);
		check_block_by_id(working_set, block_id, &mut visited_blocks, config_root)?;
	}

	Ok(())
}

fn check_block_by_id(working_set: &StateWorkingSet<'_>, block_id: BlockId, visited_blocks: &mut HashSet<BlockId>, config_root: Option<&Path>) -> Result<()> {
	if !visited_blocks.insert(block_id) {
		return Ok(());
	}

	let block = working_set.get_block(block_id);
	check_block(working_set, block, visited_blocks, config_root)
}

fn check_block(working_set: &StateWorkingSet<'_>, block: &Block, visited_blocks: &mut HashSet<BlockId>, config_root: Option<&Path>) -> Result<()> {
	for pipeline in &block.pipelines {
		for element in &pipeline.elements {
			check_expression(working_set, &element.expr, visited_blocks, config_root)?;
			if element.redirection.is_some() {
				return Err(ConfigError::NuSandbox("pipeline redirection is disabled in config.nu".to_string()));
			}
		}
	}

	Ok(())
}

fn check_expression(
	working_set: &StateWorkingSet<'_>,
	expression: &Expression,
	visited_blocks: &mut HashSet<BlockId>,
	config_root: Option<&Path>,
) -> Result<()> {
	match &expression.expr {
		Expr::ExternalCall(_, _) => Err(ConfigError::NuSandbox("external commands are disabled in config.nu".to_string())),
		Expr::Call(call) => {
			let decl_name = working_set.get_decl(call.decl_id).name();
			if is_use_decl(decl_name) {
				return check_use_call(working_set, call, visited_blocks, config_root);
			}
			if let Some(reason) = blocked_decl_reason(decl_name) {
				return Err(ConfigError::NuSandbox(format!("'{decl_name}' is not allowed in config.nu ({reason})")));
			}

			for arg in &call.arguments {
				match arg {
					Argument::Positional(expr) | Argument::Unknown(expr) | Argument::Spread(expr) => {
						check_expression(working_set, expr, visited_blocks, config_root)?;
					}
					Argument::Named((_, _, maybe_expr)) => {
						if let Some(expr) = maybe_expr {
							check_expression(working_set, expr, visited_blocks, config_root)?;
						}
					}
				}
			}
			for expr in call.parser_info.values() {
				check_expression(working_set, expr, visited_blocks, config_root)?;
			}

			Ok(())
		}
		Expr::AttributeBlock(attribute_block) => {
			for attribute in &attribute_block.attributes {
				check_expression(working_set, &attribute.expr, visited_blocks, config_root)?;
			}
			check_expression(working_set, &attribute_block.item, visited_blocks, config_root)
		}
		Expr::UnaryNot(expr) => check_expression(working_set, expr, visited_blocks, config_root),
		Expr::BinaryOp(lhs, op, rhs) => {
			check_expression(working_set, lhs, visited_blocks, config_root)?;
			check_expression(working_set, op, visited_blocks, config_root)?;
			check_expression(working_set, rhs, visited_blocks, config_root)
		}
		Expr::Collect(_, expr) => check_expression(working_set, expr, visited_blocks, config_root),
		Expr::Subexpression(block_id) | Expr::Block(block_id) | Expr::Closure(block_id) | Expr::RowCondition(block_id) => {
			check_block_by_id(working_set, *block_id, visited_blocks, config_root)
		}
		Expr::MatchBlock(cases) => {
			for (_, expr) in cases {
				check_expression(working_set, expr, visited_blocks, config_root)?;
			}
			Ok(())
		}
		Expr::List(list) => {
			for item in list {
				match item {
					ListItem::Item(expr) | ListItem::Spread(_, expr) => {
						check_expression(working_set, expr, visited_blocks, config_root)?;
					}
				}
			}
			Ok(())
		}
		Expr::Record(items) => {
			for item in items {
				match item {
					RecordItem::Pair(key, value) => {
						check_expression(working_set, key, visited_blocks, config_root)?;
						check_expression(working_set, value, visited_blocks, config_root)?;
					}
					RecordItem::Spread(_, value) => {
						check_expression(working_set, value, visited_blocks, config_root)?;
					}
				}
			}
			Ok(())
		}
		Expr::Keyword(keyword) => check_expression(working_set, &keyword.expr, visited_blocks, config_root),
		Expr::ValueWithUnit(value_with_unit) => check_expression(working_set, &value_with_unit.expr, visited_blocks, config_root),
		Expr::FullCellPath(path) => check_expression(working_set, &path.head, visited_blocks, config_root),
		Expr::GlobPattern(_, _) | Expr::GlobInterpolation(_, _) => Err(ConfigError::NuSandbox("glob expansion is disabled in config.nu".to_string())),
		Expr::StringInterpolation(items) => {
			for item in items {
				check_expression(working_set, item, visited_blocks, config_root)?;
			}
			Ok(())
		}
		Expr::Range(range) => {
			if let Some(from) = &range.from {
				check_expression(working_set, from, visited_blocks, config_root)?;
			}
			if let Some(next) = &range.next {
				check_expression(working_set, next, visited_blocks, config_root)?;
			}
			if let Some(to) = &range.to {
				check_expression(working_set, to, visited_blocks, config_root)?;
			}
			Ok(())
		}
		Expr::Table(table) => {
			for column in table.columns.iter() {
				check_expression(working_set, column, visited_blocks, config_root)?;
			}
			for row in table.rows.iter() {
				for cell in row.iter() {
					check_expression(working_set, cell, visited_blocks, config_root)?;
				}
			}
			Ok(())
		}
		_ => Ok(()),
	}
}

fn is_use_decl(decl_name: &str) -> bool {
	matches!(decl_name, "use" | "export use")
}

fn check_use_call(
	working_set: &StateWorkingSet<'_>,
	call: &nu_protocol::ast::Call,
	visited_blocks: &mut HashSet<BlockId>,
	config_root: Option<&Path>,
) -> Result<()> {
	let Some((module_index, module_expr)) = call.arguments.iter().enumerate().find_map(|(idx, arg)| match arg {
		Argument::Positional(expr) | Argument::Unknown(expr) => Some((idx, expr)),
		Argument::Named(_) | Argument::Spread(_) => None,
	}) else {
		return Err(ConfigError::NuSandbox("use requires a static module path literal".to_string()));
	};

	let raw_path = module_path_literal(module_expr).ok_or_else(|| ConfigError::NuSandbox("use module path must be a static path literal".to_string()))?;
	validate_module_path(config_root, raw_path)?;

	for (idx, arg) in call.arguments.iter().enumerate() {
		if idx == module_index {
			continue;
		}
		match arg {
			Argument::Positional(expr) | Argument::Unknown(expr) | Argument::Spread(expr) => {
				check_expression(working_set, expr, visited_blocks, config_root)?;
			}
			Argument::Named((_, _, maybe_expr)) => {
				if let Some(expr) = maybe_expr {
					check_expression(working_set, expr, visited_blocks, config_root)?;
				}
			}
		}
	}
	for expr in call.parser_info.values() {
		check_expression(working_set, expr, visited_blocks, config_root)?;
	}

	Ok(())
}

fn module_path_literal(expr: &Expression) -> Option<&str> {
	match &expr.expr {
		Expr::String(path) | Expr::Filepath(path, _) | Expr::GlobPattern(path, _) => Some(path),
		_ => None,
	}
}

fn validate_module_path(config_root: Option<&Path>, raw_path: &str) -> Result<()> {
	if raw_path.is_empty() {
		return Err(ConfigError::NuSandbox("use module path cannot be empty".to_string()));
	}
	if raw_path.contains('\0') {
		return Err(ConfigError::NuSandbox("use module path contains NUL byte".to_string()));
	}
	if raw_path.contains('~') || raw_path.contains('$') || raw_path.contains('`') {
		return Err(ConfigError::NuSandbox("use module path must not use shell expansion tokens".to_string()));
	}
	if raw_path.chars().any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}')) {
		return Err(ConfigError::NuSandbox("use module path must not contain glob patterns".to_string()));
	}

	let path = Path::new(raw_path);
	if path.is_absolute() {
		return Err(ConfigError::NuSandbox("absolute module paths are not allowed in config.nu".to_string()));
	}
	for comp in path.components() {
		match comp {
			Component::ParentDir => {
				return Err(ConfigError::NuSandbox("module paths cannot traverse parent directories".to_string()));
			}
			Component::Prefix(_) | Component::RootDir => {
				return Err(ConfigError::NuSandbox("module path has an unsupported root or prefix".to_string()));
			}
			Component::CurDir | Component::Normal(_) => {}
		}
	}
	if path.extension().and_then(|ext| ext.to_str()) != Some("nu") {
		return Err(ConfigError::NuSandbox("module path must point to a .nu file".to_string()));
	}

	let config_root = config_root.ok_or_else(|| ConfigError::NuSandbox("use requires config.nu to be loaded from a real file path".to_string()))?;
	let root_canon = std::fs::canonicalize(config_root).map_err(|e| ConfigError::NuSandbox(format!("failed to resolve config directory root: {e}")))?;
	let candidate_canon =
		std::fs::canonicalize(config_root.join(path)).map_err(|e| ConfigError::NuSandbox(format!("failed to resolve module path '{raw_path}': {e}")))?;
	if !candidate_canon.starts_with(&root_canon) {
		return Err(ConfigError::NuSandbox("module path resolves outside the config directory root".to_string()));
	}
	let metadata = std::fs::metadata(&candidate_canon).map_err(|e| ConfigError::NuSandbox(format!("failed to stat module path '{raw_path}': {e}")))?;
	if !metadata.is_file() {
		return Err(ConfigError::NuSandbox("module path must resolve to a file".to_string()));
	}

	Ok(())
}

fn blocked_decl_reason(decl_name: &str) -> Option<&'static str> {
	let name = decl_name.to_ascii_lowercase();
	match name.as_str() {
		"run-external" => Some("external execution is disabled"),
		"source" | "source-env" | "overlay use" | "overlay new" | "overlay hide" => Some("module and filesystem loading are disabled"),
		"for" | "while" | "loop" => Some("looping commands are disabled"),
		"exec" | "bash" | "sh" | "nu" | "cmd" | "powershell" | "pwsh" => Some("process execution commands are disabled"),
		"open" | "save" | "rm" | "mv" | "cp" | "mkdir" | "ls" | "cd" => Some("filesystem commands are disabled"),
		"http" | "curl" | "wget" => Some("network commands are disabled"),
		"plugin" | "register" | "plugin use" => Some("plugin commands are disabled"),
		_ => None,
	}
}

#[cfg(test)]
mod tests;
