use xeno_registry::actions::CommandError;

/// Unified status for "try to apply something" (effect, result handler, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
	/// The dispatcher successfully applied the request.
	Applied,

	/// Dispatcher refused due to invariant violation.
	Denied(CommandError),

	/// No registered handler exists for this request.
	/// `kind` MUST be stable (e.g. "EditOp::Post::RevealCursor").
	Unhandled { kind: &'static str },
}
