//! Native Nu command declarations for invocation constructors and builtins.
//!
//! Each invocation command returns a plain `Value::Record` with a `kind` field
//! identifying the invocation type. No custom values needed.

mod xeno_assert;
mod xeno_call;
mod xeno_ctx;
mod xeno_emit;
mod xeno_emit_many;
mod xeno_is_invocation;
mod xeno_log;

use xeno_nu_protocol::engine::StateWorkingSet;

/// Register all xeno invocation commands into a working set.
pub fn register_all(working_set: &mut StateWorkingSet<'_>) {
	working_set.add_decl(Box::new(xeno_call::XenoCallCommand));
	working_set.add_decl(Box::new(xeno_ctx::XenoCtxCommand));
	working_set.add_decl(Box::new(xeno_log::XenoLogCommand));
	working_set.add_decl(Box::new(xeno_assert::XenoAssertCommand));
	working_set.add_decl(Box::new(xeno_emit::XenoEmitCommand));
	working_set.add_decl(Box::new(xeno_emit_many::XenoEmitManyCommand));
	working_set.add_decl(Box::new(xeno_is_invocation::XenoIsInvocationCommand));
}
