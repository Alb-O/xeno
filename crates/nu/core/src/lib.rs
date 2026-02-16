//! Internal Nu core wrappers used by `xeno-nu-runtime`.
//!
//! Keeps direct coupling to the vendored Nu internals in one place.

use std::path::Path;

pub use xeno_nu_protocol::DeclId;
pub use xeno_nu_protocol::ast::Block;
pub use xeno_nu_protocol::engine::EngineState;
use xeno_nu_value::Value;

#[derive(Debug)]
pub struct ParseResult {
	pub block: std::sync::Arc<Block>,
	pub script_decl_ids: Vec<DeclId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsePolicy {
	Script,
	ModuleOnly,
}

impl ParsePolicy {
	fn to_sandbox(self) -> xeno_nu_sandbox::ParsePolicy {
		match self {
			Self::Script => xeno_nu_sandbox::ParsePolicy::Script,
			Self::ModuleOnly => xeno_nu_sandbox::ParsePolicy::ModuleOnly,
		}
	}
}

pub fn create_engine_state(config_root: Option<&Path>) -> Result<EngineState, String> {
	xeno_nu_sandbox::create_engine_state(config_root)
}

pub fn parse_and_validate_with_policy(
	engine_state: &mut EngineState,
	fname: &str,
	source: &str,
	config_root: Option<&Path>,
	policy: ParsePolicy,
) -> Result<ParseResult, String> {
	let parsed = xeno_nu_sandbox::parse_and_validate_with_policy(engine_state, fname, source, config_root, policy.to_sandbox())?;
	Ok(ParseResult {
		block: parsed.block,
		script_decl_ids: parsed.script_decl_ids,
	})
}

pub fn evaluate_block(engine_state: &EngineState, block: &Block) -> Result<Value, String> {
	xeno_nu_sandbox::evaluate_block(engine_state, block)
}

pub fn find_decl(engine_state: &EngineState, name: &str) -> Option<DeclId> {
	xeno_nu_sandbox::find_decl(engine_state, name)
}

pub fn call_function(engine_state: &EngineState, decl_id: DeclId, args: &[String], env: &[(&str, Value)]) -> Result<Value, String> {
	xeno_nu_sandbox::call_function(engine_state, decl_id, args, env)
}

pub fn call_function_owned(engine_state: &EngineState, decl_id: DeclId, args: Vec<String>, env: Vec<(String, Value)>) -> Result<Value, String> {
	xeno_nu_sandbox::call_function_owned(engine_state, decl_id, args, env)
}

pub fn eval_source_with_policy(fname: &str, source: &str, config_root: Option<&Path>, policy: ParsePolicy) -> Result<Value, String> {
	let mut engine_state = create_engine_state(config_root)?;
	let parsed = parse_and_validate_with_policy(&mut engine_state, fname, source, config_root, policy)?;
	evaluate_block(&engine_state, parsed.block.as_ref())
}
