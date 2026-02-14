//! AST-level sandbox validation for Nu scripts.
//!
//! Walks the parsed AST and rejects constructs that could escape the sandbox:
//! external commands, filesystem I/O, networking, looping, and module paths
//! that resolve outside the config root directory.

use std::collections::HashSet;
use std::path::{Component, Path};

use nu_protocol::ast::{Argument, Block, Expr, Expression, ListItem, RecordItem};
use nu_protocol::engine::StateWorkingSet;
use nu_protocol::BlockId;

/// Validates that a parsed working set contains no sandbox violations.
///
/// Walks the root block and all newly-parsed delta blocks. Returns an error
/// string describing the first violation found.
pub fn ensure_sandboxed(
	working_set: &StateWorkingSet<'_>,
	root: &Block,
	config_root: Option<&Path>,
) -> Result<(), String> {
	let mut visited = HashSet::new();
	check_block(working_set, root, &mut visited, config_root)?;

	let base = working_set.permanent_state.num_blocks();
	for idx in 0..working_set.delta.blocks.len() {
		let block_id = BlockId::new(base + idx);
		check_block_by_id(working_set, block_id, &mut visited, config_root)?;
	}

	Ok(())
}

fn check_block_by_id(
	working_set: &StateWorkingSet<'_>,
	block_id: BlockId,
	visited: &mut HashSet<BlockId>,
	config_root: Option<&Path>,
) -> Result<(), String> {
	if !visited.insert(block_id) {
		return Ok(());
	}
	check_block(working_set, working_set.get_block(block_id), visited, config_root)
}

fn check_block(
	working_set: &StateWorkingSet<'_>,
	block: &Block,
	visited: &mut HashSet<BlockId>,
	config_root: Option<&Path>,
) -> Result<(), String> {
	for pipeline in &block.pipelines {
		for element in &pipeline.elements {
			check_expression(working_set, &element.expr, visited, config_root)?;
			if element.redirection.is_some() {
				return Err("pipeline redirection is disabled".to_string());
			}
		}
	}
	Ok(())
}

fn check_expression(
	working_set: &StateWorkingSet<'_>,
	expression: &Expression,
	visited: &mut HashSet<BlockId>,
	config_root: Option<&Path>,
) -> Result<(), String> {
	match &expression.expr {
		Expr::ExternalCall(_, _) => Err("external commands are disabled".to_string()),

		Expr::Call(call) => {
			let decl_name = working_set.get_decl(call.decl_id).name();
			if is_use_decl(decl_name) {
				return check_use_call(working_set, call, visited, config_root);
			}
			if let Some(reason) = blocked_decl_reason(decl_name) {
				return Err(format!("'{decl_name}' is not allowed ({reason})"));
			}

			for arg in &call.arguments {
				match arg {
					Argument::Positional(expr) | Argument::Unknown(expr) | Argument::Spread(expr) => {
						check_expression(working_set, expr, visited, config_root)?;
					}
					Argument::Named((_, _, maybe_expr)) => {
						if let Some(expr) = maybe_expr {
							check_expression(working_set, expr, visited, config_root)?;
						}
					}
				}
			}
			for expr in call.parser_info.values() {
				check_expression(working_set, expr, visited, config_root)?;
			}
			Ok(())
		}

		Expr::AttributeBlock(ab) => {
			for attr in &ab.attributes {
				check_expression(working_set, &attr.expr, visited, config_root)?;
			}
			check_expression(working_set, &ab.item, visited, config_root)
		}

		Expr::UnaryNot(expr) => check_expression(working_set, expr, visited, config_root),

		Expr::BinaryOp(lhs, op, rhs) => {
			check_expression(working_set, lhs, visited, config_root)?;
			check_expression(working_set, op, visited, config_root)?;
			check_expression(working_set, rhs, visited, config_root)
		}

		Expr::Collect(_, expr) => check_expression(working_set, expr, visited, config_root),

		Expr::Subexpression(id) | Expr::Block(id) | Expr::Closure(id) | Expr::RowCondition(id) => {
			check_block_by_id(working_set, *id, visited, config_root)
		}

		Expr::MatchBlock(cases) => {
			for (_, expr) in cases {
				check_expression(working_set, expr, visited, config_root)?;
			}
			Ok(())
		}

		Expr::List(list) => {
			for item in list {
				match item {
					ListItem::Item(expr) | ListItem::Spread(_, expr) => {
						check_expression(working_set, expr, visited, config_root)?;
					}
				}
			}
			Ok(())
		}

		Expr::Record(items) => {
			for item in items {
				match item {
					RecordItem::Pair(key, value) => {
						check_expression(working_set, key, visited, config_root)?;
						check_expression(working_set, value, visited, config_root)?;
					}
					RecordItem::Spread(_, value) => {
						check_expression(working_set, value, visited, config_root)?;
					}
				}
			}
			Ok(())
		}

		Expr::Keyword(kw) => check_expression(working_set, &kw.expr, visited, config_root),
		Expr::ValueWithUnit(vu) => check_expression(working_set, &vu.expr, visited, config_root),
		Expr::FullCellPath(path) => check_expression(working_set, &path.head, visited, config_root),

		Expr::GlobPattern(_, _) | Expr::GlobInterpolation(_, _) => {
			Err("glob expansion is disabled".to_string())
		}

		Expr::StringInterpolation(items) => {
			for item in items {
				check_expression(working_set, item, visited, config_root)?;
			}
			Ok(())
		}

		Expr::Range(range) => {
			if let Some(from) = &range.from {
				check_expression(working_set, from, visited, config_root)?;
			}
			if let Some(next) = &range.next {
				check_expression(working_set, next, visited, config_root)?;
			}
			if let Some(to) = &range.to {
				check_expression(working_set, to, visited, config_root)?;
			}
			Ok(())
		}

		Expr::Table(table) => {
			for col in table.columns.iter() {
				check_expression(working_set, col, visited, config_root)?;
			}
			for row in table.rows.iter() {
				for cell in row.iter() {
					check_expression(working_set, cell, visited, config_root)?;
				}
			}
			Ok(())
		}

		_ => Ok(()),
	}
}

// --- `use` statement validation ---

fn is_use_decl(decl_name: &str) -> bool {
	matches!(decl_name, "use" | "export use")
}

fn check_use_call(
	working_set: &StateWorkingSet<'_>,
	call: &nu_protocol::ast::Call,
	visited: &mut HashSet<BlockId>,
	config_root: Option<&Path>,
) -> Result<(), String> {
	let Some((module_index, module_expr)) = call
		.arguments
		.iter()
		.enumerate()
		.find_map(|(idx, arg)| match arg {
			Argument::Positional(expr) | Argument::Unknown(expr) => Some((idx, expr)),
			_ => None,
		})
	else {
		return Err("use requires a static module path literal".to_string());
	};

	let raw_path = module_path_literal(module_expr)
		.ok_or_else(|| "use module path must be a static path literal".to_string())?;
	validate_module_path(config_root, raw_path)?;

	for (idx, arg) in call.arguments.iter().enumerate() {
		if idx == module_index {
			continue;
		}
		match arg {
			Argument::Positional(expr) | Argument::Unknown(expr) | Argument::Spread(expr) => {
				check_expression(working_set, expr, visited, config_root)?;
			}
			Argument::Named((_, _, maybe_expr)) => {
				if let Some(expr) = maybe_expr {
					check_expression(working_set, expr, visited, config_root)?;
				}
			}
		}
	}
	for expr in call.parser_info.values() {
		check_expression(working_set, expr, visited, config_root)?;
	}
	Ok(())
}

fn module_path_literal(expr: &Expression) -> Option<&str> {
	match &expr.expr {
		Expr::String(path) | Expr::Filepath(path, _) | Expr::GlobPattern(path, _) => Some(path),
		_ => None,
	}
}

fn validate_module_path(config_root: Option<&Path>, raw_path: &str) -> Result<(), String> {
	if raw_path.is_empty() {
		return Err("use module path cannot be empty".to_string());
	}
	if raw_path.contains('\0') {
		return Err("use module path contains NUL byte".to_string());
	}
	if raw_path.contains('~') || raw_path.contains('$') || raw_path.contains('`') {
		return Err("use module path must not use shell expansion tokens".to_string());
	}
	if raw_path
		.chars()
		.any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}'))
	{
		return Err("use module path must not contain glob patterns".to_string());
	}

	let path = Path::new(raw_path);
	if path.is_absolute() {
		return Err("absolute module paths are not allowed".to_string());
	}
	for comp in path.components() {
		match comp {
			Component::ParentDir => {
				return Err("module paths cannot traverse parent directories".to_string());
			}
			Component::Prefix(_) | Component::RootDir => {
				return Err("module path has an unsupported root or prefix".to_string());
			}
			Component::CurDir | Component::Normal(_) => {}
		}
	}
	if path.extension().and_then(|ext| ext.to_str()) != Some("nu") {
		return Err("module path must point to a .nu file".to_string());
	}

	let config_root = config_root
		.ok_or_else(|| "use requires a real config directory path".to_string())?;
	let root_canon = std::fs::canonicalize(config_root)
		.map_err(|e| format!("failed to resolve config directory root: {e}"))?;
	let candidate_canon = std::fs::canonicalize(config_root.join(path))
		.map_err(|e| format!("failed to resolve module path '{raw_path}': {e}"))?;
	if !candidate_canon.starts_with(&root_canon) {
		return Err("module path resolves outside the config directory root".to_string());
	}
	let metadata = std::fs::metadata(&candidate_canon)
		.map_err(|e| format!("failed to stat module path '{raw_path}': {e}"))?;
	if !metadata.is_file() {
		return Err("module path must resolve to a file".to_string());
	}

	Ok(())
}

// --- Blocked command list ---

fn blocked_decl_reason(decl_name: &str) -> Option<&'static str> {
	let name = decl_name.to_ascii_lowercase();
	match name.as_str() {
		"run-external" => Some("external execution is disabled"),
		"source" | "source-env" | "overlay use" | "overlay new" | "overlay hide" => {
			Some("module and filesystem loading are disabled")
		}
		"for" | "while" | "loop" => Some("looping commands are disabled"),
		"exec" | "bash" | "sh" | "nu" | "cmd" | "powershell" | "pwsh" => {
			Some("process execution commands are disabled")
		}
		"open" | "save" | "rm" | "mv" | "cp" | "mkdir" | "ls" | "cd" => {
			Some("filesystem commands are disabled")
		}
		"http" | "curl" | "wget" => Some("network commands are disabled"),
		"plugin" | "register" | "plugin use" => Some("plugin commands are disabled"),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sandbox_check(source: &str, config_root: Option<&Path>) -> Result<(), String> {
		let mut engine_state = crate::create_engine_state(config_root);
		let mut working_set = nu_protocol::engine::StateWorkingSet::new(&engine_state);
		let block = nu_parser::parse(&mut working_set, Some("<test>"), source.as_bytes(), false);
		if let Some(err) = working_set.parse_errors.first() {
			return Err(format!("parse error: {err}"));
		}
		ensure_sandboxed(&working_set, block.as_ref(), config_root)?;
		let delta = working_set.render();
		engine_state.merge_delta(delta).map_err(|e| e.to_string())?;
		Ok(())
	}

	#[test]
	fn blocks_external_command() {
		let err = sandbox_check("^ls", None).unwrap_err();
		assert!(err.contains("external") || err.contains("parse error"), "{err}");
	}

	#[test]
	fn blocks_run_external() {
		let err = sandbox_check("run-external 'ls'", None).unwrap_err();
		assert!(err.contains("external") || err.contains("parse error"), "{err}");
	}

	#[test]
	fn blocks_filesystem_commands() {
		for cmd in ["open foo.txt", "save bar.txt", "rm baz", "cp a b", "ls", "cd /tmp", "mkdir d"] {
			let result = sandbox_check(cmd, None);
			assert!(result.is_err(), "{cmd} should be blocked: {result:?}");
		}
	}

	#[test]
	fn blocks_networking() {
		let err = sandbox_check("http get https://example.com", None).unwrap_err();
		assert!(err.contains("network") || err.contains("external") || err.contains("not allowed") || err.contains("parse error"), "{err}");
	}

	#[test]
	fn blocks_looping() {
		for cmd in ["while true { }", "for x in [1 2] { }", "loop { break }"] {
			let err = sandbox_check(cmd, None).unwrap_err();
			assert!(err.contains("looping") || err.contains("parse error"), "{cmd}: {err}");
		}
	}

	#[test]
	fn blocks_redirection() {
		let err = sandbox_check("1 | save out.txt", None).unwrap_err();
		assert!(err.contains("not allowed") || err.contains("filesystem") || err.contains("external") || err.contains("parse error"), "{err}");
	}

	#[test]
	fn allows_pure_record() {
		sandbox_check("{ name: 'test', value: 42 }", None).expect("pure record should pass");
	}

	#[test]
	fn allows_function_defs() {
		sandbox_check("def greet [name: string] { $'hello ($name)' }\ngreet 'world'", None)
			.expect("function defs should pass");
	}

	#[test]
	fn blocks_parent_dir_in_use() {
		let temp = tempfile::tempdir().unwrap();
		let err = sandbox_check("use ../evil.nu", Some(temp.path())).unwrap_err();
		assert!(err.contains("parent") || err.contains("parse error"), "{err}");
	}

	#[test]
	fn allows_use_within_root() {
		let temp = tempfile::tempdir().unwrap();
		std::fs::write(temp.path().join("helper.nu"), "export def x [] { 1 }").unwrap();
		sandbox_check("use helper.nu", Some(temp.path())).expect("use within root should pass");
	}
}
