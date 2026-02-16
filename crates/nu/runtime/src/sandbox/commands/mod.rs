//! Native Nu command declarations for invocation constructors and builtins.
//!
//! Each invocation command returns a plain `Value::Record` with a `kind` field
//! identifying the invocation type. No custom values needed.

mod action;
mod command;
mod editor;
mod nu_run;
mod xeno_assert;
mod xeno_ctx;
mod xeno_emit;
mod xeno_emit_many;
mod xeno_is_invocation;
mod xeno_log;

use xeno_nu_protocol::engine::StateWorkingSet;
use xeno_nu_protocol::{ShellError, Span};

/// Register all xeno invocation commands into a working set.
pub fn register_all(working_set: &mut StateWorkingSet<'_>) {
	working_set.add_decl(Box::new(action::ActionCommand));
	working_set.add_decl(Box::new(command::CommandCommand));
	working_set.add_decl(Box::new(editor::EditorCommand));
	working_set.add_decl(Box::new(nu_run::NuRunCommand));
	working_set.add_decl(Box::new(xeno_ctx::XenoCtxCommand));
	working_set.add_decl(Box::new(xeno_log::XenoLogCommand));
	working_set.add_decl(Box::new(xeno_assert::XenoAssertCommand));
	working_set.add_decl(Box::new(xeno_emit::XenoEmitCommand));
	working_set.add_decl(Box::new(xeno_emit_many::XenoEmitManyCommand));
	working_set.add_decl(Box::new(xeno_is_invocation::XenoIsInvocationCommand));
}

fn parse_single_char(value: Option<String>, flag: &str, span: Span) -> Result<Option<char>, ShellError> {
	let Some(s) = value else { return Ok(None) };
	let mut chars = s.chars();
	let Some(ch) = chars.next() else {
		return Err(ShellError::GenericError {
			error: format!("--{flag} must be exactly one character"),
			msg: "empty string".into(),
			span: Some(span),
			help: None,
			inner: vec![],
		});
	};
	if chars.next().is_some() {
		return Err(ShellError::GenericError {
			error: format!("--{flag} must be exactly one character"),
			msg: format!("got '{}' ({} chars)", s, s.chars().count()),
			span: Some(span),
			help: None,
			inner: vec![],
		});
	}
	Ok(Some(ch))
}
