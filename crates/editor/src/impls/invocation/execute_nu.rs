use tracing::debug;
use xeno_nu_data::Value;

use crate::impls::Editor;
use crate::impls::invocation::kernel::InvocationKernel;
use crate::nu::NuDecodeSurface;
use crate::nu::coordinator::errors::exec_error_message;
use crate::nu::effects::{NuEffectApplyError, NuEffectApplyMode, apply_effect_batch};
use crate::types::{Invocation, InvocationOutcome, InvocationPolicy, InvocationTarget};

impl Editor {
	pub(crate) async fn run_nu_macro_invocation(&mut self, fn_name: String, args: Vec<String>) -> Result<Vec<Invocation>, InvocationOutcome> {
		if let Err(error) = self.ensure_nu_runtime_loaded().await {
			let mut kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
			return Err(kernel.command_error_with_notification(InvocationTarget::Nu, error));
		}

		let Some(decl_id) = self.state.integration.nu.resolve_macro_decl_cached(&fn_name) else {
			let error = format!("Nu runtime error: function '{}' is not defined in xeno.nu", fn_name);
			let mut kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
			return Err(kernel.command_error_with_notification(InvocationTarget::Nu, error));
		};

		let Some(executor) = self.state.integration.nu.ensure_executor() else {
			let kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
			return Err(kernel.command_error(InvocationTarget::Nu, "Nu executor is not available"));
		};
		let executor_client = executor.client();

		let budget = self
			.state
			.config
			.nu
			.as_ref()
			.map_or_else(crate::nu::DecodeBudget::macro_defaults, |config| config.macro_decode_budget());
		let nu_ctx = self.build_nu_ctx("macro", &fn_name, true);
		let env = vec![("XENO_CTX".to_string(), nu_ctx)];

		let host = self.build_nu_host_snapshot();

		let effects = match executor_client
			.run(decl_id, NuDecodeSurface::Macro, args, budget, env, Some(Box::new(host)))
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

		let allowed = self.state.config.config.nu.as_ref().map_or_else(
			|| xeno_registry::config::NuConfig::default().macro_permissions(),
			|config| config.macro_permissions(),
		);

		let outcome = match apply_effect_batch(self, effects, NuEffectApplyMode::Macro, &allowed) {
			Ok(outcome) => outcome,
			Err(error) => {
				let msg = match error {
					NuEffectApplyError::PermissionDenied { permission } => {
						format!("Nu macro effect denied by permission policy: {}", permission.as_str())
					}
					NuEffectApplyError::StopPropagationUnsupportedForMacro => "Nu macro produced hook-only stop effect".to_string(),
				};
				let mut kernel = InvocationKernel::new(self, InvocationPolicy::enforcing());
				return Err(kernel.command_error_with_notification(InvocationTarget::Nu, msg));
			}
		};

		Ok(outcome.dispatches)
	}

	pub(crate) fn build_nu_host_snapshot(&self) -> crate::nu::host::NuHostSnapshot {
		let buffer = self.buffer();
		let (rope, line_count) = buffer.with_doc(|doc| {
			let content = doc.content();
			(content.clone(), content.len_lines())
		});
		let meta = xeno_nu_api::BufferMeta {
			path: buffer.path().map(|p| p.to_string_lossy().to_string()),
			file_type: buffer.file_type(),
			readonly: buffer.is_readonly(),
			modified: buffer.modified(),
			line_count,
		};
		crate::nu::host::NuHostSnapshot::new(meta, rope)
	}

	async fn ensure_nu_runtime_loaded(&mut self) -> Result<(), String> {
		if self.nu_runtime().is_some() {
			return Ok(());
		}

		let config_dir = crate::paths::get_config_dir().ok_or_else(|| "config directory is unavailable; cannot auto-load xeno.nu".to_string())?;
		let loaded = xeno_worker::spawn_blocking(xeno_worker::TaskClass::CpuBlocking, move || crate::nu::NuRuntime::load(&config_dir))
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

	/// Build the `$env.XENO_CTX` value for a Nu macro invocation.
	///
	/// Populates the `text` record with the current cursor line and selection
	/// content, each clamped to the byte budget. Event is always `None`.
	pub(crate) fn build_nu_ctx(&self, kind: &str, function: &str, include_text: bool) -> Value {
		self.build_nu_ctx_inner(kind, function, include_text, None)
	}

	/// Build the `$env.XENO_CTX` value for a hook invocation.
	///
	/// Skips text extraction (hooks don't get buffer text snapshots).
	/// Injects the event record so scripts can dispatch via `$env.XENO_CTX.event.type`.
	pub(crate) fn build_nu_hook_ctx(&self, event: &crate::nu::ctx::NuCtxEvent) -> Value {
		self.build_nu_ctx_inner("hook", "on_hook", false, Some(event.clone()))
	}

	fn build_nu_ctx_inner(&self, kind: &str, function: &str, include_text: bool, event: Option<crate::nu::ctx::NuCtxEvent>) -> Value {
		use crate::nu::ctx::{
			NuCtx, NuCtxBuffer, NuCtxPosition, NuCtxRange, NuCtxSelection, NuCtxText, NuCtxView, TEXT_SNAPSHOT_MAX_BYTES, rope_slice_clamped,
		};

		let buffer = self.buffer();
		let view_id = self.focused_view().0;
		let primary_selection = buffer.selection.primary();
		let primary_index = buffer.selection.primary_index();
		let cursor_char = buffer.cursor;
		let sel_active = !primary_selection.is_point();

		let (cursor_line, cursor_col, sel_start_line, sel_start_col, sel_end_line, sel_end_col, ctx_ranges, text_snapshot) = buffer.with_doc(|doc| {
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

			let ranges: Vec<NuCtxRange> = buffer
				.selection
				.ranges()
				.iter()
				.map(|r| {
					let (al, ac) = to_line_col(r.anchor);
					let (hl, hc) = to_line_col(r.head);
					NuCtxRange {
						anchor: NuCtxPosition { line: al, col: ac },
						head: NuCtxPosition { line: hl, col: hc },
					}
				})
				.collect();

			let snapshot = if include_text {
				let line_slice = text.line(cl);
				let (mut line_str, line_trunc) = rope_slice_clamped(line_slice, TEXT_SNAPSHOT_MAX_BYTES);
				let trimmed_len = line_str.trim_end_matches('\n').trim_end_matches('\r').len();
				line_str.truncate(trimmed_len);

				let (sel_str, sel_trunc) = if sel_active {
					let sel_min = primary_selection.min().min(text.len_chars());
					let sel_max = primary_selection.max().min(text.len_chars());
					let sel_slice = text.slice(sel_min..sel_max);
					let (s, t) = rope_slice_clamped(sel_slice, TEXT_SNAPSHOT_MAX_BYTES);
					(Some(s), t)
				} else {
					(None, false)
				};

				NuCtxText {
					line: Some(line_str),
					line_truncated: line_trunc,
					selection: sel_str,
					selection_truncated: sel_trunc,
				}
			} else {
				NuCtxText::empty()
			};

			(cl, cc, ssl, ssc, sel, sec, ranges, snapshot)
		});

		let state_snapshot: Vec<(String, String)> = self
			.state
			.core
			.editor
			.workspace
			.nu_state
			.iter()
			.map(|(k, v)| (k.to_string(), v.to_string()))
			.collect();

		NuCtx {
			kind: kind.to_string(),
			function: function.to_string(),
			mode: format!("{:?}", self.mode()),
			view: NuCtxView { id: view_id },
			cursor: NuCtxPosition {
				line: cursor_line,
				col: cursor_col,
			},
			selection: NuCtxSelection {
				active: sel_active,
				primary: primary_index,
				start: NuCtxPosition {
					line: sel_start_line,
					col: sel_start_col,
				},
				end: NuCtxPosition {
					line: sel_end_line,
					col: sel_end_col,
				},
				ranges: ctx_ranges,
			},
			buffer: NuCtxBuffer {
				path: buffer.path().map(|path| path.to_string_lossy().to_string()),
				file_type: buffer.file_type(),
				readonly: buffer.is_readonly(),
				modified: buffer.modified(),
			},
			text: text_snapshot,
			event,
			state: state_snapshot,
		}
		.to_value()
	}
}
