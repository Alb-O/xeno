use tracing::debug;
use xeno_nu_data::Value;

use crate::impls::Editor;
use crate::impls::invocation::kernel::InvocationKernel;
use crate::nu::NuDecodeSurface;
use crate::nu::coordinator::errors::exec_error_message;
use crate::nu::coordinator::runner::{NuExecKind, execute_with_restart};
use crate::nu::effects::{NuEffectApplyError, NuEffectApplyMode, apply_effect_batch};
use crate::types::{Invocation, InvocationOutcome, InvocationPolicy, InvocationTarget};

impl Editor {
	pub(crate) async fn run_nu_macro_invocation(&mut self, fn_name: String, args: Vec<String>) -> Result<Vec<Invocation>, InvocationOutcome> {
		if let Err(error) = self.ensure_nu_runtime_loaded().await {
			let mut kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
			return Err(kernel.command_error_with_notification(InvocationTarget::Nu, error));
		}

		let Some(runtime) = self.nu_runtime() else {
			let kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
			return Err(kernel.command_error(InvocationTarget::Nu, "Nu runtime is not loaded"));
		};

		let Some(decl_id) = runtime.find_script_decl(&fn_name) else {
			let error = format!("Nu runtime error: function '{}' is not defined in xeno.nu", fn_name);
			let mut kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
			return Err(kernel.command_error_with_notification(InvocationTarget::Nu, error));
		};

		if self.state.nu.ensure_executor().is_none() {
			let kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
			return Err(kernel.command_error(InvocationTarget::Nu, "Nu executor is not available"));
		}

		let budget = self
			.state
			.config
			.nu
			.as_ref()
			.map_or_else(crate::nu::DecodeBudget::macro_defaults, |config| config.macro_decode_budget());
		let nu_ctx = self.build_nu_ctx("macro", &fn_name, &args);

		let effects = match execute_with_restart(
			&mut self.state.nu,
			NuExecKind::Macro,
			&fn_name,
			decl_id,
			args,
			NuDecodeSurface::Macro,
			budget,
			nu_ctx,
		)
		.await
		{
			Ok(effects) => effects,
			Err(error) => {
				let msg = exec_error_message(&error);
				let mut kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
				return Err(kernel.command_error_with_notification(InvocationTarget::Nu, msg));
			}
		};

		if effects.effects.is_empty() {
			debug!(function = %fn_name, "Nu macro produced no invocations");
			return Ok(Vec::new());
		}

		let allowed = self.state.config.nu.as_ref().map_or_else(
			|| xeno_registry::config::NuConfig::default().macro_capabilities(),
			|config| config.macro_capabilities(),
		);

		let outcome = match apply_effect_batch(self, effects, NuEffectApplyMode::Macro, &allowed) {
			Ok(outcome) => outcome,
			Err(error) => {
				let msg = match error {
					NuEffectApplyError::CapabilityDenied { capability } => {
						format!("Nu macro effect denied by capability policy: {}", capability.as_str())
					}
					NuEffectApplyError::StopPropagationUnsupportedForMacro => "Nu macro produced hook-only stop effect".to_string(),
				};
				let mut kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
				return Err(kernel.command_error_with_notification(InvocationTarget::Nu, msg));
			}
		};

		Ok(outcome.dispatches)
	}

	async fn ensure_nu_runtime_loaded(&mut self) -> Result<(), String> {
		if self.nu_runtime().is_some() {
			return Ok(());
		}

		let config_dir = crate::paths::get_config_dir().ok_or_else(|| "config directory is unavailable; cannot auto-load xeno.nu".to_string())?;
		let loaded = tokio::task::spawn_blocking(move || crate::nu::NuRuntime::load(&config_dir))
			.await
			.map_err(|error| format!("failed to join Nu runtime load task: {error}"))?;

		match loaded {
			Ok(runtime) => {
				self.set_nu_runtime(Some(runtime));
				Ok(())
			}
			Err(error) => Err(error),
		}
	}

	pub(crate) fn build_nu_ctx(&self, kind: &str, function: &str, args: &[String]) -> Value {
		use crate::nu::ctx::{NuCtx, NuCtxBuffer, NuCtxPosition, NuCtxSelection, NuCtxView};

		let buffer = self.buffer();
		let view_id = self.focused_view().0;
		let primary_selection = buffer.selection.primary();
		let cursor_char = buffer.cursor;

		let (cursor_line, cursor_col, sel_start_line, sel_start_col, sel_end_line, sel_end_col) = buffer.with_doc(|doc| {
			let text = doc.content();
			let to_line_col = |idx: usize| {
				let clamped = idx.min(text.len_chars());
				let line = text.char_to_line(clamped);
				let col = clamped.saturating_sub(text.line_to_char(line));
				(line, col)
			};

			let (cl, cc) = to_line_col(cursor_char);
			let (ssl, ssc) = to_line_col(primary_selection.min());
			let (sel, sec) = to_line_col(primary_selection.max());
			(cl, cc, ssl, ssc, sel, sec)
		});

		NuCtx {
			kind: kind.to_string(),
			function: function.to_string(),
			args: args.to_vec(),
			mode: format!("{:?}", self.mode()),
			view: NuCtxView { id: view_id },
			cursor: NuCtxPosition {
				line: cursor_line,
				col: cursor_col,
			},
			selection: NuCtxSelection {
				active: !primary_selection.is_point(),
				start: NuCtxPosition {
					line: sel_start_line,
					col: sel_start_col,
				},
				end: NuCtxPosition {
					line: sel_end_line,
					col: sel_end_col,
				},
			},
			buffer: NuCtxBuffer {
				path: buffer.path().map(|path| path.to_string_lossy().to_string()),
				file_type: buffer.file_type(),
				readonly: buffer.is_readonly(),
				modified: buffer.modified(),
			},
		}
		.to_value()
	}
}
