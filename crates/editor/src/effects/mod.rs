pub mod sink;
pub mod types;

use xeno_registry::actions::editor_ctx::{OverlayAccess, OverlayRequest};
use xeno_registry::commands::CommandError;

use crate::effects::sink::DrainedEffects;

impl crate::impls::Editor {
	/// Flushes all pending effects from the sink and applies them.
	///
	/// Re-entrant calls are deferred: nested flushes signal a redraw and
	/// return immediately, letting the outermost flush complete first.
	pub fn flush_effects(&mut self) {
		if self.state.flush_depth > 0 {
			self.state.frame.needs_redraw = true;
			return;
		}

		self.state.flush_depth += 1;

		loop {
			let drained = self.state.effects.drain();
			if drained.is_empty() {
				break;
			}
			self.apply_drained_effects(drained);
		}

		self.state.flush_depth -= 1;
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
			let mut layers = std::mem::take(&mut self.state.overlay_system.layers);
			for e in eff.layer_events {
				layers.notify_event(self, e);
			}
			self.state.overlay_system.layers = layers;
		}

		if !eff.notifications.is_empty() {
			needs_redraw = true;
			for n in eff.notifications {
				self.notify(n);
			}
		}

		for (name, args) in eff.queued_commands {
			self.state.core.workspace.command_queue.push(name, args);
		}

		if needs_redraw {
			self.state.frame.needs_redraw = true;
		}
	}

	/// Dispatches a single [`OverlayRequest`] to the overlay system.
	///
	/// Commit closes are deferred via [`crate::types::FrameState::pending_overlay_commit`]
	/// because [`crate::overlay::OverlayController::on_commit`] is async and cannot run inside
	/// the synchronous effect flush loop.
	pub(crate) fn handle_overlay_request(
		&mut self,
		req: OverlayRequest,
	) -> Result<(), CommandError> {
		use xeno_registry::actions::editor_ctx::OverlayCloseReason;
		use xeno_registry::actions::editor_ctx::OverlayRequest::*;

		match req {
			OpenModal { kind, args } => {
				match kind {
					"command_palette" => {
						self.open_command_palette();
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
						self.state.frame.pending_overlay_commit = true;
					}
					reason => {
						let reason = match reason {
							OverlayCloseReason::Cancel => CloseReason::Cancel,
							OverlayCloseReason::Blur => CloseReason::Blur,
							OverlayCloseReason::Forced => CloseReason::Forced,
							OverlayCloseReason::Commit => unreachable!(),
						};
						let mut interaction =
							std::mem::take(&mut self.state.overlay_system.interaction);
						interaction.close(self, reason);
						self.state.overlay_system.interaction = interaction;
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

impl OverlayAccess for crate::impls::Editor {
	fn overlay_request(&mut self, req: OverlayRequest) -> Result<(), CommandError> {
		self.handle_overlay_request(req)
	}

	fn overlay_modal_is_open(&self) -> bool {
		self.state.overlay_system.interaction.is_open()
	}
}
