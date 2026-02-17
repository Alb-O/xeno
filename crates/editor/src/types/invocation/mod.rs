//! Invocation types for unified action/command dispatch.
//!
//! The canonical [`Invocation`] enum lives in `xeno_registry` and is re-exported
//! here for convenience. Editor-specific dispatch types (`InvocationPolicy`,
//! `InvocationOutcome`) remain local.

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationTarget {
	Action,
	Command,
	Nu,
}

/// Terminal status of an invocation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationStatus {
	Ok,
	Quit,
	ForceQuit,
	NotFound,
	CapabilityDenied,
	ReadonlyDenied,
	CommandError,
}

/// Structured detail payload for invocation outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationDetail {
	NotFoundTarget(String),
	Message(String),
	Capability(Capability),
}

/// Structured invocation outcome with explicit status and diagnostics payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationOutcome {
	pub status: InvocationStatus,
	pub target: InvocationTarget,
	pub detail: Option<InvocationDetail>,
}

impl InvocationOutcome {
	pub const fn ok(target: InvocationTarget) -> Self {
		Self {
			status: InvocationStatus::Ok,
			target,
			detail: None,
		}
	}

	pub const fn quit(target: InvocationTarget) -> Self {
		Self {
			status: InvocationStatus::Quit,
			target,
			detail: None,
		}
	}

	pub const fn force_quit(target: InvocationTarget) -> Self {
		Self {
			status: InvocationStatus::ForceQuit,
			target,
			detail: None,
		}
	}

	pub fn not_found(target: InvocationTarget, detail: impl Into<String>) -> Self {
		Self {
			status: InvocationStatus::NotFound,
			target,
			detail: Some(InvocationDetail::NotFoundTarget(detail.into())),
		}
	}

	pub const fn capability_denied(target: InvocationTarget, capability: Capability) -> Self {
		Self {
			status: InvocationStatus::CapabilityDenied,
			target,
			detail: Some(InvocationDetail::Capability(capability)),
		}
	}

	pub const fn readonly_denied(target: InvocationTarget) -> Self {
		Self {
			status: InvocationStatus::ReadonlyDenied,
			target,
			detail: None,
		}
	}

	pub fn command_error(target: InvocationTarget, detail: impl Into<String>) -> Self {
		Self {
			status: InvocationStatus::CommandError,
			target,
			detail: Some(InvocationDetail::Message(detail.into())),
		}
	}

	/// Returns true if this result indicates a quit request.
	pub fn is_quit(&self) -> bool {
		matches!(self.status, InvocationStatus::Quit | InvocationStatus::ForceQuit)
	}

	/// Returns true if this result indicates successful execution.
	pub fn is_ok(&self) -> bool {
		matches!(self.status, InvocationStatus::Ok)
	}

	pub fn detail_text(&self) -> Option<&str> {
		match &self.detail {
			Some(InvocationDetail::NotFoundTarget(text) | InvocationDetail::Message(text)) => Some(text.as_str()),
			Some(InvocationDetail::Capability(_)) | None => None,
		}
	}

	pub const fn denied_capability(&self) -> Option<Capability> {
		match self.detail {
			Some(InvocationDetail::Capability(capability)) => Some(capability),
			Some(InvocationDetail::NotFoundTarget(_) | InvocationDetail::Message(_)) | None => None,
		}
	}

	pub fn label(&self) -> &'static str {
		match self.status {
			InvocationStatus::Ok => "ok",
			InvocationStatus::Quit => "quit",
			InvocationStatus::ForceQuit => "force_quit",
			InvocationStatus::NotFound => "not_found",
			InvocationStatus::CapabilityDenied => "cap_denied",
			InvocationStatus::ReadonlyDenied => "readonly",
			InvocationStatus::CommandError => "error",
		}
	}
}
