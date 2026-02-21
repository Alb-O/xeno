//! Invocation types for unified action/command dispatch.
//!
//! The canonical [`Invocation`] enum lives in `xeno_registry` and is re-exported
//! here for convenience. Editor-specific dispatch types (`InvocationPolicy`,
//! `InvocationOutcome`) remain local.

pub use xeno_registry::Invocation;

pub(crate) mod adapters;

/// Policy for readonly enforcement during invocation dispatch.
#[derive(Debug, Clone, Copy)]
pub struct InvocationPolicy {
	/// Whether to check and enforce readonly buffer status.
	///
	/// * `true`: Block edits to readonly buffers
	/// * `false`: Allow (useful for testing)
	pub enforce_readonly: bool,
}

impl Default for InvocationPolicy {
	fn default() -> Self {
		Self::log_only()
	}
}

impl InvocationPolicy {
	/// Creates a policy that doesn't block execution.
	pub const fn log_only() -> Self {
		Self { enforce_readonly: false }
	}

	/// Creates a policy that enforces all checks.
	pub const fn enforcing() -> Self {
		Self { enforce_readonly: true }
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
	ReadonlyDenied,
	CommandError,
}

/// Structured detail payload for invocation outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationDetail {
	NotFoundTarget(String),
	Message(String),
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
			None => None,
		}
	}

	pub fn label(&self) -> &'static str {
		match self.status {
			InvocationStatus::Ok => "ok",
			InvocationStatus::Quit => "quit",
			InvocationStatus::ForceQuit => "force_quit",
			InvocationStatus::NotFound => "not_found",
			InvocationStatus::ReadonlyDenied => "readonly",
			InvocationStatus::CommandError => "error",
		}
	}
}
