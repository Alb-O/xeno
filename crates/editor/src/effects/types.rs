use xeno_registry::Capability;
use xeno_registry::actions::CommandError;

/// Unified status for "try to apply something" (effect, result handler, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
	/// The dispatcher successfully applied the request.
	Applied,

	/// Dispatcher refused due to missing capability / invariant.
	/// Keep this typed; do not lose information.
	Denied(CommandError),

	/// No registered handler exists for this request.
	/// `kind` MUST be stable (e.g. "EditOp::Post::RevealCursor").
	Unhandled { kind: &'static str },
}

impl DispatchOutcome {
	#[inline]
	pub fn denied_cap(cap: Capability) -> Self {
		DispatchOutcome::Denied(CommandError::PermissionDenied(cap))
	}
}
