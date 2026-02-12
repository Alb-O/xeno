//! Nu script configuration parsing for Xeno.

use std::collections::HashSet;

use nu_protocol::ast::{Argument, Block, Expr, Expression, ListItem, RecordItem};
use nu_protocol::engine::{Stack, StateWorkingSet};
use nu_protocol::{BlockId, PipelineData, Span};

use super::{Config, ConfigError, Result};

/// Evaluate a Nu script and parse its resulting value as [`Config`].
pub fn eval_config_str(input: &str, fname: &str) -> Result<Config> {
	let mut engine_state = nu_cmd_lang::create_default_context();
	let mut working_set = StateWorkingSet::new(&engine_state);
	let block = nu_parser::parse(&mut working_set, Some(fname), input.as_bytes(), false);

	if let Some(error) = working_set.parse_errors.first() {
		return Err(ConfigError::NuParse(error.to_string()));
	}
	if let Some(error) = working_set.compile_errors.first() {
		return Err(ConfigError::NuParse(error.to_string()));
	}

	ensure_working_set_is_sandboxed(&working_set, block.as_ref())?;

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

fn ensure_working_set_is_sandboxed(working_set: &StateWorkingSet<'_>, root: &Block) -> Result<()> {
	let mut visited_blocks = HashSet::new();
	check_block(working_set, root, &mut visited_blocks)?;

	let base = working_set.permanent_state.num_blocks();
	for idx in 0..working_set.delta.blocks.len() {
		let block_id = BlockId::new(base + idx);
		check_block_by_id(working_set, block_id, &mut visited_blocks)?;
	}

	Ok(())
}

fn check_block_by_id(working_set: &StateWorkingSet<'_>, block_id: BlockId, visited_blocks: &mut HashSet<BlockId>) -> Result<()> {
	if !visited_blocks.insert(block_id) {
		return Ok(());
	}

	let block = working_set.get_block(block_id);
	check_block(working_set, block, visited_blocks)
}

fn check_block(working_set: &StateWorkingSet<'_>, block: &Block, visited_blocks: &mut HashSet<BlockId>) -> Result<()> {
	for pipeline in &block.pipelines {
		for element in &pipeline.elements {
			check_expression(working_set, &element.expr, visited_blocks)?;
			if element.redirection.is_some() {
				return Err(ConfigError::NuSandbox("pipeline redirection is disabled in config.nu".to_string()));
			}
		}
	}

	Ok(())
}

fn check_expression(working_set: &StateWorkingSet<'_>, expression: &Expression, visited_blocks: &mut HashSet<BlockId>) -> Result<()> {
	match &expression.expr {
		Expr::ExternalCall(_, _) => Err(ConfigError::NuSandbox("external commands are disabled in config.nu".to_string())),
		Expr::Call(call) => {
			let decl_name = working_set.get_decl(call.decl_id).name();
			if let Some(reason) = blocked_decl_reason(decl_name) {
				return Err(ConfigError::NuSandbox(format!("'{decl_name}' is not allowed in config.nu ({reason})")));
			}

			for arg in &call.arguments {
				match arg {
					Argument::Positional(expr) | Argument::Unknown(expr) | Argument::Spread(expr) => {
						check_expression(working_set, expr, visited_blocks)?;
					}
					Argument::Named((_, _, maybe_expr)) => {
						if let Some(expr) = maybe_expr {
							check_expression(working_set, expr, visited_blocks)?;
						}
					}
				}
			}
			for expr in call.parser_info.values() {
				check_expression(working_set, expr, visited_blocks)?;
			}

			Ok(())
		}
		Expr::AttributeBlock(attribute_block) => {
			for attribute in &attribute_block.attributes {
				check_expression(working_set, &attribute.expr, visited_blocks)?;
			}
			check_expression(working_set, &attribute_block.item, visited_blocks)
		}
		Expr::UnaryNot(expr) => check_expression(working_set, expr, visited_blocks),
		Expr::BinaryOp(lhs, op, rhs) => {
			check_expression(working_set, lhs, visited_blocks)?;
			check_expression(working_set, op, visited_blocks)?;
			check_expression(working_set, rhs, visited_blocks)
		}
		Expr::Collect(_, expr) => check_expression(working_set, expr, visited_blocks),
		Expr::Subexpression(block_id) | Expr::Block(block_id) | Expr::Closure(block_id) | Expr::RowCondition(block_id) => {
			check_block_by_id(working_set, *block_id, visited_blocks)
		}
		Expr::MatchBlock(cases) => {
			for (_, expr) in cases {
				check_expression(working_set, expr, visited_blocks)?;
			}
			Ok(())
		}
		Expr::List(list) => {
			for item in list {
				match item {
					ListItem::Item(expr) | ListItem::Spread(_, expr) => {
						check_expression(working_set, expr, visited_blocks)?;
					}
				}
			}
			Ok(())
		}
		Expr::Record(items) => {
			for item in items {
				match item {
					RecordItem::Pair(key, value) => {
						check_expression(working_set, key, visited_blocks)?;
						check_expression(working_set, value, visited_blocks)?;
					}
					RecordItem::Spread(_, value) => {
						check_expression(working_set, value, visited_blocks)?;
					}
				}
			}
			Ok(())
		}
		Expr::Keyword(keyword) => check_expression(working_set, &keyword.expr, visited_blocks),
		Expr::ValueWithUnit(value_with_unit) => check_expression(working_set, &value_with_unit.expr, visited_blocks),
		Expr::FullCellPath(path) => check_expression(working_set, &path.head, visited_blocks),
		Expr::GlobPattern(_, _) | Expr::GlobInterpolation(_, _) => Err(ConfigError::NuSandbox("glob expansion is disabled in config.nu".to_string())),
		Expr::StringInterpolation(items) => {
			for item in items {
				check_expression(working_set, item, visited_blocks)?;
			}
			Ok(())
		}
		Expr::Range(range) => {
			if let Some(from) = &range.from {
				check_expression(working_set, from, visited_blocks)?;
			}
			if let Some(next) = &range.next {
				check_expression(working_set, next, visited_blocks)?;
			}
			if let Some(to) = &range.to {
				check_expression(working_set, to, visited_blocks)?;
			}
			Ok(())
		}
		Expr::Table(table) => {
			for column in table.columns.iter() {
				check_expression(working_set, column, visited_blocks)?;
			}
			for row in table.rows.iter() {
				for cell in row.iter() {
					check_expression(working_set, cell, visited_blocks)?;
				}
			}
			Ok(())
		}
		_ => Ok(()),
	}
}

fn blocked_decl_reason(decl_name: &str) -> Option<&'static str> {
	let name = decl_name.to_ascii_lowercase();
	match name.as_str() {
		"run-external" => Some("external execution is disabled"),
		"use" | "source" | "source-env" | "overlay use" | "overlay new" | "overlay hide" | "export use" => Some("module and filesystem loading are disabled"),
		"for" | "while" | "loop" => Some("looping commands are disabled"),
		"exec" | "bash" | "sh" | "nu" | "cmd" | "powershell" | "pwsh" => Some("process execution commands are disabled"),
		"open" | "save" | "rm" | "mv" | "cp" | "mkdir" | "ls" | "cd" => Some("filesystem commands are disabled"),
		"http" | "curl" | "wget" => Some("network commands are disabled"),
		"plugin" | "register" | "plugin use" => Some("plugin commands are disabled"),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn eval_config_returns_config() {
		let config = eval_config_str("{ options: { tab-width: 4 } }", "config.nu").expect("config.nu should evaluate");
		let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
		assert_eq!(config.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(4)));
	}

	#[test]
	fn eval_config_rejects_external() {
		let err = eval_config_str("^echo hi; { options: { tab-width: 4 } }", "config.nu").expect_err("external commands must be rejected");
		assert!(matches!(err, ConfigError::NuSandbox(_) | ConfigError::NuParse(_)));
	}

	#[test]
	fn eval_config_rejects_redirection() {
		let err = eval_config_str("1 > out.txt; { options: { tab-width: 4 } }", "config.nu").expect_err("redirection must be rejected");
		assert!(matches!(err, ConfigError::NuSandbox(_) | ConfigError::NuParse(_)));
	}

	#[test]
	fn eval_config_rejects_while() {
		let err = eval_config_str("while true { }; { options: { tab-width: 4 } }", "config.nu").expect_err("while loops must be rejected");
		assert!(matches!(err, ConfigError::NuSandbox(_)));
	}

	#[test]
	fn eval_config_merge_precedence() {
		let mut merged = crate::config::kdl::parse_config_str("options { tab-width 2 }").expect("kdl config should parse");
		merged.merge(crate::config::nuon::parse_config_str("{ options: { tab-width: 3 } }").expect("nuon config should parse"));
		merged.merge(eval_config_str("{ options: { tab-width: 4 } }", "config.nu").expect("nu config should evaluate"));

		let tab_width = crate::options::find("tab-width").expect("tab-width option should exist");
		assert_eq!(merged.options.get(tab_width.dense_id()), Some(&crate::options::OptionValue::Int(4)));
	}
}
