use std::time::{Duration, Instant};

use tracing::{debug, warn};
use xeno_invocation::nu::DecodeBudget;
use xeno_nu_api::ExportId;
use xeno_nu_data::Value;

use super::state::NuCoordinatorState;
use crate::nu::executor::NuExecError;
use crate::nu::{NuDecodeSurface, NuEffectBatch};

const SLOW_NU_HOOK_THRESHOLD: Duration = Duration::from_millis(2);
const SLOW_NU_MACRO_THRESHOLD: Duration = Duration::from_millis(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NuExecKind {
	#[cfg_attr(not(test), allow(dead_code))]
	Hook,
	Macro,
}

impl NuExecKind {
	fn label(self) -> &'static str {
		match self {
			Self::Hook => "hook",
			Self::Macro => "macro",
		}
	}

	fn slow_threshold(self) -> Duration {
		match self {
			Self::Hook => SLOW_NU_HOOK_THRESHOLD,
			Self::Macro => SLOW_NU_MACRO_THRESHOLD,
		}
	}
}

/// Run a Nu function on the executor with automatic restart-and-retry on shutdown.
///
/// Handles `Shutdown` (retry once after restart), `ReplyDropped` (restart,
/// no retry), and `Eval` errors uniformly for both hooks and macros.
pub(crate) async fn execute_with_restart(
	nu: &mut NuCoordinatorState,
	kind: NuExecKind,
	fn_name: &str,
	decl_id: ExportId,
	args: Vec<String>,
	surface: NuDecodeSurface,
	budget: DecodeBudget,
	nu_ctx: Value,
) -> Result<NuEffectBatch, NuExecError> {
	nu.ensure_executor();
	let Some(executor) = nu.executor_client() else {
		return Err(NuExecError::Eval("Nu executor is not available".to_string()));
	};

	let start = Instant::now();
	let env = vec![("XENO_CTX".to_string(), nu_ctx)];

	let effects = match executor.run(decl_id, surface, args, budget, env).await {
		Ok(effects) => effects,
		Err(NuExecError::Shutdown {
			decl_id,
			surface,
			args,
			budget,
			env,
		}) => {
			warn!(label = kind.label(), function = fn_name, "Nu executor died, restarting");
			nu.restart_executor();
			nu.ensure_executor();
			let Some(retry_executor) = nu.executor_client() else {
				return Err(NuExecError::Eval("Nu executor could not be restarted".to_string()));
			};
			retry_executor.run(decl_id, surface, args, budget, env).await?
		}
		Err(NuExecError::ReplyDropped) => {
			warn!(label = kind.label(), function = fn_name, "Nu executor worker died mid-evaluation, restarting");
			nu.restart_executor();
			return Err(NuExecError::ReplyDropped);
		}
		Err(error) => return Err(error),
	};

	let elapsed = start.elapsed();
	if elapsed > kind.slow_threshold() {
		debug!(
			label = kind.label(),
			function = fn_name,
			elapsed_ms = elapsed.as_millis() as u64,
			"slow Nu call"
		);
	}

	Ok(effects)
}
