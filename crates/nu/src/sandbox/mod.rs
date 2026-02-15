//! AST-level sandbox validation for Nu scripts.
//!
//! Walks the parsed AST and rejects structural constructs that cannot be
//! prevented by engine context alone: external commands (`^cmd`),
//! pipeline redirection, glob expansion, and module paths that resolve
//! outside the config root directory.
//!
//! Most dangerous commands (loops, overlays, extern signatures) are excluded
//! at the engine level by [`create_xeno_lang_context`](super::create_xeno_lang_context)
//! and fail at parse/compile time. This module provides defense-in-depth for
//! `source`/`source-env` and handles structural AST escapes.
//!
//! Policy model:
//! * Xeno intentionally reuses Nushell parser semantics for `use`/`export use`
//!   import patterns and module resolution, then applies a root-confinement
//!   check over parser-resolved files.
//! * This module does not reimplement Nu import grammar.
//! * The root-confinement check is canonicalized and rejects symlink escapes.
//!
//! Primary sources:
//! * Nushell Book, Modules: <https://www.nushell.sh/book/modules/using_modules.html>
//! * Nushell Book, Parse/Eval model: <https://www.nushell.sh/book/how_nushell_code_gets_run.html>
//! * Nu parser module resolution (`parse_module_file_or_dir`):
//!   <https://github.com/nushell/nushell/blob/main/crates/nu-parser/src/parse_keywords.rs>
//! * Nu parser import-pattern parsing (`parse_import_pattern`):
//!   <https://github.com/nushell/nushell/blob/main/crates/nu-parser/src/parser.rs>

use std::collections::HashSet;
use std::path::Path;

use nu_protocol::BlockId;
use nu_protocol::ast::{Argument, Block, Expr, Expression, ListItem, MatchPattern, Pattern, RecordItem};
use nu_protocol::engine::StateWorkingSet;

/// Validates that a parsed working set contains no sandbox violations.
///
/// Walks the root block and all newly-parsed delta blocks. Returns an error
/// string describing the first violation found.
pub fn ensure_sandboxed(working_set: &StateWorkingSet<'_>, root: &Block, config_root: Option<&Path>) -> Result<(), String> {
	let mut visited = HashSet::new();
	let mut state = SandboxScanState::default();
	check_block(working_set, root, &mut visited, &mut state)?;

	let base = working_set.permanent_state.num_blocks();
	for idx in 0..working_set.delta.blocks.len() {
		let block_id = BlockId::new(base + idx);
		check_block_by_id(working_set, block_id, &mut visited, &mut state)?;
	}

	if state.saw_use {
		validate_resolved_module_paths(working_set, config_root)?;
	}

	Ok(())
}

#[derive(Default)]
struct SandboxScanState {
	saw_use: bool,
}

fn check_block_by_id(working_set: &StateWorkingSet<'_>, block_id: BlockId, visited: &mut HashSet<BlockId>, state: &mut SandboxScanState) -> Result<(), String> {
	if !visited.insert(block_id) {
		return Ok(());
	}
	check_block(working_set, working_set.get_block(block_id), visited, state)
}

fn check_block(working_set: &StateWorkingSet<'_>, block: &Block, visited: &mut HashSet<BlockId>, state: &mut SandboxScanState) -> Result<(), String> {
	for pipeline in &block.pipelines {
		for element in &pipeline.elements {
			check_expression(working_set, &element.expr, visited, state)?;
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
	state: &mut SandboxScanState,
) -> Result<(), String> {
	match &expression.expr {
		Expr::ExternalCall(_, _) => Err("external commands are disabled".to_string()),
		Expr::Filepath(_, _) | Expr::Directory(_, _) => Err("filesystem path literals are disabled".to_string()),

		Expr::Call(call) => {
			let decl_name = working_set.get_decl(call.decl_id).name();
			if is_use_decl(decl_name) {
				state.saw_use = true;
				// `use` import patterns are parsed/validated by Nushell parser semantics.
				return Ok(());
			}
			// Defense-in-depth: reject `source`/`source-env` if they somehow
			// appear despite not being registered in the engine context.
			if is_source_decl(decl_name) {
				return Err(format!("'{decl_name}' is not allowed (source loading is disabled)"));
			}

			for arg in &call.arguments {
				match arg {
					Argument::Positional(expr) | Argument::Unknown(expr) | Argument::Spread(expr) => {
						check_expression(working_set, expr, visited, state)?;
					}
					Argument::Named((_, _, maybe_expr)) => {
						if let Some(expr) = maybe_expr {
							check_expression(working_set, expr, visited, state)?;
						}
					}
				}
			}
			for expr in call.parser_info.values() {
				check_expression(working_set, expr, visited, state)?;
			}
			Ok(())
		}

		Expr::AttributeBlock(ab) => {
			for attr in &ab.attributes {
				check_expression(working_set, &attr.expr, visited, state)?;
			}
			check_expression(working_set, &ab.item, visited, state)
		}

		Expr::UnaryNot(expr) => check_expression(working_set, expr, visited, state),

		Expr::BinaryOp(lhs, op, rhs) => {
			check_expression(working_set, lhs, visited, state)?;
			check_expression(working_set, op, visited, state)?;
			check_expression(working_set, rhs, visited, state)
		}

		Expr::Collect(_, expr) => check_expression(working_set, expr, visited, state),

		Expr::Subexpression(id) | Expr::Block(id) | Expr::Closure(id) | Expr::RowCondition(id) => check_block_by_id(working_set, *id, visited, state),

		Expr::MatchBlock(cases) => {
			for (pattern, expr) in cases {
				check_match_pattern(working_set, pattern, visited, state)?;
				check_expression(working_set, expr, visited, state)?;
			}
			Ok(())
		}

		Expr::List(list) => {
			for item in list {
				match item {
					ListItem::Item(expr) | ListItem::Spread(_, expr) => {
						check_expression(working_set, expr, visited, state)?;
					}
				}
			}
			Ok(())
		}

		Expr::Record(items) => {
			for item in items {
				match item {
					RecordItem::Pair(key, value) => {
						check_expression(working_set, key, visited, state)?;
						check_expression(working_set, value, visited, state)?;
					}
					RecordItem::Spread(_, value) => {
						check_expression(working_set, value, visited, state)?;
					}
				}
			}
			Ok(())
		}

		Expr::Keyword(kw) => check_expression(working_set, &kw.expr, visited, state),
		Expr::ValueWithUnit(vu) => check_expression(working_set, &vu.expr, visited, state),
		Expr::FullCellPath(path) => check_expression(working_set, &path.head, visited, state),

		Expr::GlobPattern(_, _) | Expr::GlobInterpolation(_, _) => Err("glob expansion is disabled".to_string()),

		Expr::StringInterpolation(items) => {
			for item in items {
				check_expression(working_set, item, visited, state)?;
			}
			Ok(())
		}

		Expr::Range(range) => {
			if let Some(from) = &range.from {
				check_expression(working_set, from, visited, state)?;
			}
			if let Some(next) = &range.next {
				check_expression(working_set, next, visited, state)?;
			}
			if let Some(to) = &range.to {
				check_expression(working_set, to, visited, state)?;
			}
			Ok(())
		}

		Expr::Table(table) => {
			for col in table.columns.iter() {
				check_expression(working_set, col, visited, state)?;
			}
			for row in table.rows.iter() {
				for cell in row.iter() {
					check_expression(working_set, cell, visited, state)?;
				}
			}
			Ok(())
		}

		Expr::Bool(_)
		| Expr::Int(_)
		| Expr::Float(_)
		| Expr::Binary(_)
		| Expr::Var(_)
		| Expr::VarDecl(_)
		| Expr::Operator(_)
		| Expr::DateTime(_)
		| Expr::String(_)
		| Expr::RawString(_)
		| Expr::CellPath(_)
		| Expr::ImportPattern(_)
		| Expr::Overlay(_)
		| Expr::Signature(_)
		| Expr::Nothing
		| Expr::Garbage => Ok(()),
	}
}

fn check_match_pattern(
	working_set: &StateWorkingSet<'_>,
	pattern: &MatchPattern,
	visited: &mut HashSet<BlockId>,
	state: &mut SandboxScanState,
) -> Result<(), String> {
	match &pattern.pattern {
		Pattern::Expression(expr) => check_expression(working_set, expr, visited, state)?,
		Pattern::List(patterns) | Pattern::Or(patterns) => {
			for pattern in patterns {
				check_match_pattern(working_set, pattern, visited, state)?;
			}
		}
		Pattern::Record(entries) => {
			for (_, pattern) in entries {
				check_match_pattern(working_set, pattern, visited, state)?;
			}
		}
		Pattern::Value(_) | Pattern::Variable(_) | Pattern::Rest(_) | Pattern::IgnoreRest | Pattern::IgnoreValue | Pattern::Garbage => {}
	}

	if let Some(guard) = &pattern.guard {
		check_expression(working_set, guard, visited, state)?;
	}

	Ok(())
}

// --- `use` statement tracking ---

fn is_use_decl(decl_name: &str) -> bool {
	matches!(decl_name, "use" | "export use")
}

fn validate_resolved_module_paths(working_set: &StateWorkingSet<'_>, config_root: Option<&Path>) -> Result<(), String> {
	let config_root = config_root.ok_or_else(|| "use requires a real config directory path".to_string())?;
	let root_canon = std::fs::canonicalize(config_root).map_err(|e| format!("failed to resolve config directory root: {e}"))?;

	for file in working_set.files() {
		let name = file.name.as_ref();
		if is_virtual_filename(name) {
			continue;
		}

		let path = Path::new(name);
		if !path.exists() {
			continue;
		}

		let candidate_canon = std::fs::canonicalize(path).map_err(|e| format!("failed to resolve module path '{name}': {e}"))?;
		if !candidate_canon.starts_with(&root_canon) {
			return Err("module path resolves outside the config directory root".to_string());
		}
	}

	Ok(())
}

fn is_virtual_filename(name: &str) -> bool {
	name.starts_with('<') && name.ends_with('>')
}

// --- Defense-in-depth checks ---

fn is_source_decl(decl_name: &str) -> bool {
	matches!(decl_name, "source" | "source-env")
}

#[cfg(test)]
mod tests;
