//! Invocation types for unified action/command dispatch.
//!
//! The canonical [`Invocation`] enum lives in `xeno_registry` and is re-exported
//! here for convenience. Editor-specific dispatch types (`InvocationPolicy`,
//! `InvocationResult`) remain local.

use xeno_registry::Capability;
pub use xeno_registry::Invocation;

/// Policy for capability enforcement during invocation dispatch.
///
/// Controls whether violations block execution or just log warnings.
/// Use log-only mode during migration, then flip to enforcing.
#[derive(Debug, Clone, Copy)]
pub struct InvocationPolicy {
	/// Whether to check and enforce required capabilities.
	///
	/// * `true`: Block execution if capabilities are missing (enforcement mode)
	/// * `false`: Log violations but continue (log-only mode)
	pub enforce_caps: bool,

	/// Whether to check and enforce readonly buffer status.
	///
	/// * `true`: Block edits to readonly buffers
	/// * `false`: Log but allow (useful for testing)
	pub enforce_readonly: bool,
}

impl Default for InvocationPolicy {
	fn default() -> Self {
		Self::log_only()
	}
}

impl InvocationPolicy {
	/// Creates a policy that logs violations but doesn't block execution.
	///
	/// Use this during migration to identify capability gaps.
	pub const fn log_only() -> Self {
		Self {
			enforce_caps: false,
			enforce_readonly: false,
		}
	}

	/// Creates a policy that enforces all checks.
	///
	/// Use this once capability gating is fully wired.
	pub const fn enforcing() -> Self {
		Self {
			enforce_caps: true,
			enforce_readonly: true,
		}
	}

	/// Creates a policy that enforces capabilities but not readonly.
	pub const fn enforce_caps_only() -> Self {
		Self {
			enforce_caps: true,
			enforce_readonly: false,
		}
	}
}

/// Result of an invocation attempt.
#[derive(Debug)]
pub enum InvocationResult {
	/// Invocation executed successfully.
	Ok,
	/// Invocation requested application quit.
	Quit,
	/// Invocation requested force quit (no prompts).
	ForceQuit,
	/// The invocation target was not found.
	NotFound(String),
	/// Capability check failed.
	CapabilityDenied(Capability),
	/// Buffer is readonly.
	ReadonlyDenied,
	/// Command execution failed with error.
	CommandError(String),
}

impl InvocationResult {
	/// Returns true if this result indicates a quit request.
	pub fn is_quit(&self) -> bool {
		matches!(self, Self::Quit | Self::ForceQuit)
	}

	/// Returns true if this result indicates successful execution.
	pub fn is_ok(&self) -> bool {
		matches!(self, Self::Ok)
	}
}
