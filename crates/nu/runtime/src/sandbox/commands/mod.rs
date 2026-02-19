//! Native Nu command declarations for typed effect constructors and builtins.

mod xeno_assert;
mod xeno_call;
mod xeno_ctx;
mod xeno_effect;
mod xeno_effects_normalize;
mod xeno_is_effect;
mod xeno_log;
mod xeno_selection_get;

use xeno_nu_protocol::engine::StateWorkingSet;
use xeno_nu_protocol::{ShellError, Span};

pub(super) fn err(span: Span, error: impl Into<String>, msg: impl Into<String>) -> ShellError {
	ShellError::GenericError {
		error: error.into(),
		msg: msg.into(),
		span: Some(span),
		help: None,
		inner: vec![],
	}
}

pub(super) fn err_help(span: Span, error: impl Into<String>, msg: impl Into<String>, help: impl Into<String>) -> ShellError {
	ShellError::GenericError {
		error: error.into(),
		msg: msg.into(),
		span: Some(span),
		help: Some(help.into()),
		inner: vec![],
	}
}

/// Register all xeno invocation commands into a working set.
pub fn register_all(working_set: &mut StateWorkingSet<'_>) {
	working_set.add_decl(Box::new(xeno_call::XenoCallCommand));
	working_set.add_decl(Box::new(xeno_ctx::XenoCtxCommand));
	working_set.add_decl(Box::new(xeno_log::XenoLogCommand));
	working_set.add_decl(Box::new(xeno_assert::XenoAssertCommand));
	working_set.add_decl(Box::new(xeno_effect::XenoEffectCommand));
	working_set.add_decl(Box::new(xeno_effects_normalize::XenoEffectsNormalizeCommand));
	working_set.add_decl(Box::new(xeno_is_effect::XenoIsEffectCommand));
	working_set.add_decl(Box::new(xeno_selection_get::XenoSelectionGetCommand));
}
