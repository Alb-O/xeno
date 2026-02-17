use tracing::warn;

use crate::commands::{CommandError, CommandOutcome};
use crate::types::{InvocationOutcome, InvocationStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PipelineDisposition {
	Continue,
	ShouldQuit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum PipelineLogContext<'a> {
	HookDrain,
	HookSync { hook: &'a str },
}

pub(crate) fn to_command_outcome_for_nu_run(outcome: &InvocationOutcome, describe: &str) -> Result<CommandOutcome, CommandError> {
	match outcome.status {
		InvocationStatus::Ok => Ok(CommandOutcome::Ok),
		InvocationStatus::Quit => Ok(CommandOutcome::Quit),
		InvocationStatus::ForceQuit => Ok(CommandOutcome::ForceQuit),
		InvocationStatus::NotFound => {
			let target = outcome.detail_text().unwrap_or("unknown");
			Err(CommandError::Failed(format!("nu-run invocation not found: {target} ({describe})")))
		}
		InvocationStatus::CapabilityDenied => {
			let cap = outcome.denied_capability();
			Err(CommandError::Failed(format!("nu-run invocation denied by capability {cap:?} ({describe})")))
		}
		InvocationStatus::ReadonlyDenied => Err(CommandError::Failed(format!("nu-run invocation blocked by readonly mode ({describe})"))),
		InvocationStatus::CommandError => {
			let error = outcome.detail_text().unwrap_or("unknown");
			Err(CommandError::Failed(format!("nu-run invocation failed: {error} ({describe})")))
		}
	}
}

pub(crate) fn classify_for_nu_pipeline(outcome: &InvocationOutcome) -> PipelineDisposition {
	match outcome.status {
		InvocationStatus::Quit | InvocationStatus::ForceQuit => PipelineDisposition::ShouldQuit,
		InvocationStatus::Ok
		| InvocationStatus::NotFound
		| InvocationStatus::CapabilityDenied
		| InvocationStatus::ReadonlyDenied
		| InvocationStatus::CommandError => PipelineDisposition::Continue,
	}
}

pub(crate) fn log_pipeline_non_ok(outcome: &InvocationOutcome, context: PipelineLogContext<'_>) {
	match outcome.status {
		InvocationStatus::Ok | InvocationStatus::Quit | InvocationStatus::ForceQuit => {}
		InvocationStatus::NotFound => {
			let target = outcome.detail_text().unwrap_or("unknown");
			match context {
				PipelineLogContext::HookDrain => {
					warn!(context = "hook_drain", target = %target, "Nu hook invocation not found");
				}
				PipelineLogContext::HookSync { hook } => {
					warn!(context = "hook_sync", hook = %hook, target = %target, "Nu hook invocation not found");
				}
			}
		}
		InvocationStatus::CapabilityDenied => {
			let cap = outcome.denied_capability();
			match context {
				PipelineLogContext::HookDrain => {
					warn!(context = "hook_drain", capability = ?cap, "Nu hook invocation denied by capability");
				}
				PipelineLogContext::HookSync { hook } => {
					warn!(context = "hook_sync", hook = %hook, capability = ?cap, "Nu hook invocation denied by capability");
				}
			}
		}
		InvocationStatus::ReadonlyDenied => match context {
			PipelineLogContext::HookDrain => {
				warn!(context = "hook_drain", "Nu hook invocation denied by readonly mode");
			}
			PipelineLogContext::HookSync { hook } => {
				warn!(context = "hook_sync", hook = %hook, "Nu hook invocation denied by readonly mode");
			}
		},
		InvocationStatus::CommandError => {
			let error = outcome.detail_text().unwrap_or("unknown");
			match context {
				PipelineLogContext::HookDrain => {
					warn!(context = "hook_drain", error = %error, "Nu hook invocation failed");
				}
				PipelineLogContext::HookSync { hook } => {
					warn!(context = "hook_sync", hook = %hook, error = %error, "Nu hook invocation failed");
				}
			}
		}
	}
}
