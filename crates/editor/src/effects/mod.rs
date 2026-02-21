//! Effect sink application boundary.
//!
//! Drains queued editor effects, applies side effects in deterministic order,
//! and routes overlay requests/events through validated capability boundaries.

pub mod sink;

#[cfg(test)]
mod invariants;

use xeno_registry::actions::editor_ctx::OverlayRequest;
use xeno_registry::commands::CommandError;

use crate::effects::sink::DrainedEffects;
use crate::runtime::work_queue::RuntimeWorkSource;

impl crate::Editor {
	/// Flushes all pending effects from the sink and applies them.
	///
	/// Re-entrant calls are deferred: nested flushes signal a redraw and
	/// return immediately, letting the outermost flush complete first.
	///
	/// # Invariants
	///
	/// * Must route all UI consequences through `EffectSink` and `flush_effects`.
	pub fn flush_effects(&mut self) {
		if self.state.runtime.flush_depth > 0 {
			self.state.core.frame.needs_redraw = true;
			return;
		}

		self.state.runtime.flush_depth += 1;

		loop {
			let drained = self.state.runtime.effects.drain();
			if drained.is_empty() {
				break;
			}
			self.apply_drained_effects(drained);
		}

		self.state.runtime.flush_depth -= 1;
	}

	fn apply_drained_effects(&mut self, eff: DrainedEffects) {
		let mut needs_redraw = eff.wants_redraw;

		if !eff.overlay_requests.is_empty() {
			needs_redraw = true;
			for req in eff.overlay_requests {
				if let Err(e) = self.handle_overlay_request(req) {
					tracing::warn!(error = ?e, "Overlay request failed");
				}
			}
		}

		if !eff.layer_events.is_empty() {
			needs_redraw = true;
			let mut layers = std::mem::take(self.state.ui.overlay_system.layers_mut());
			for e in eff.layer_events {
				layers.notify_event(self, e);
			}
			*self.state.ui.overlay_system.layers_mut() = layers;
		}

		if !eff.notifications.is_empty() {
			needs_redraw = true;
			for n in eff.notifications {
				self.notify(n);
			}
		}

		for request in eff.queued_invocation_requests {
			self.enqueue_runtime_invocation_request(request, RuntimeWorkSource::ActionEffect);
		}

		if needs_redraw {
			self.state.core.frame.needs_redraw = true;
		}
	}

	/// Validates an [`OverlayRequest`] for correctness without applying it.
	///
	/// Use this for synchronous error reporting at the capability boundary.
	pub(crate) fn validate_overlay_request(&self, req: &OverlayRequest) -> Result<(), CommandError> {
		use xeno_registry::actions::editor_ctx::OverlayRequest::*;

		match req {
			OpenModal { kind, .. } => match *kind {
				"command_palette" | "search" | "file_picker" => Ok(()),
				_ => Err(CommandError::NotFound(format!("modal:{kind}"))),
			},
			CloseModal { .. } => Ok(()),
			ShowInfoPopup { .. } => Ok(()),
		}
	}

	/// Dispatches a single [`OverlayRequest`] to the overlay system.
	///
	/// Commit closes are deferred by queueing runtime overlay-commit work because
	/// [`crate::overlay::OverlayController::on_commit`] is async and cannot run
	/// inside the synchronous effect flush loop.
	pub(crate) fn handle_overlay_request(&mut self, req: OverlayRequest) -> Result<(), CommandError> {
		use xeno_registry::actions::editor_ctx::OverlayCloseReason;
		use xeno_registry::actions::editor_ctx::OverlayRequest::*;

		match req {
			OpenModal { kind, args } => {
				match kind {
					"command_palette" => {
						self.open_command_palette();
					}
					"file_picker" => {
						self.open_file_picker();
					}
					"search" => {
						let reverse = args.first().is_some_and(|s| s == "true");
						self.open_search(reverse);
					}
					_ => {
						tracing::warn!(kind, ?args, "Unknown modal kind requested");
						return Err(CommandError::NotFound(format!("modal:{kind}")));
					}
				}
				Ok(())
			}
			CloseModal { reason } => {
				use crate::overlay::CloseReason;
				match reason {
					OverlayCloseReason::Commit => {
						self.enqueue_runtime_overlay_commit_work();
					}
					reason => {
						let reason = match reason {
							OverlayCloseReason::Cancel => CloseReason::Cancel,
							OverlayCloseReason::Blur => CloseReason::Blur,
							OverlayCloseReason::Forced => CloseReason::Forced,
							OverlayCloseReason::Commit => unreachable!(),
						};
						let mut interaction = self.state.ui.overlay_system.take_interaction();
						interaction.close(self, reason);
						self.state.ui.overlay_system.restore_interaction(interaction);
					}
				}
				Ok(())
			}
			ShowInfoPopup { title: _, body } => {
				use crate::info_popup::PopupAnchor;
				self.open_info_popup(body, None, PopupAnchor::Center);
				Ok(())
			}
		}
	}
}
