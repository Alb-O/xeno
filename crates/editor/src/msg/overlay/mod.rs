//! Overlay-related async messages.
//!
//! Rename results are token-gated: the controller mints a monotonic token on
//! submit, and `RenameDone` carries that token through. Apply validates the
//! token against `pending_rename_token` before enqueuing the workspace edit,
//! ensuring stale or superseded rename results are silently dropped.

use xeno_registry::notifications::Notification;

use super::Dirty;
use crate::Editor;

/// Messages for async overlay outcomes.
pub enum OverlayMsg {
	/// Emit a user notification.
	Notify(Notification),
	/// Queue a workspace edit to be applied in runtime pump.
	#[cfg(feature = "lsp")]
	ApplyWorkspaceEdit(xeno_lsp::lsp_types::WorkspaceEdit),
	/// Rename preparation completed with a token-gated result.
	///
	/// If valid, updates the rename overlay with the server-provided range
	/// and placeholder. If invalid, closes the overlay with a notification.
	#[cfg(feature = "lsp")]
	RenamePrepared {
		token: u64,
		result: Result<Option<xeno_lsp::lsp_types::PrepareRenameResponse>, String>,
		encoding: xeno_lsp::OffsetEncoding,
		/// The initial prompt text set when the overlay opened, used to detect
		/// whether the user has edited the prompt before the prepare response arrives.
		expected_prompt: String,
	},
	/// Rename RPC completed with a token-gated result.
	///
	/// Only applied if `token` matches the pending rename token in editor
	/// state, preventing stale rename results from mutating buffers.
	#[cfg(feature = "lsp")]
	RenameDone {
		token: u64,
		result: Result<Option<xeno_lsp::lsp_types::WorkspaceEdit>, String>,
	},
}

impl std::fmt::Debug for OverlayMsg {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Notify(n) => f.debug_tuple("Notify").field(n).finish(),
			#[cfg(feature = "lsp")]
			Self::ApplyWorkspaceEdit(_) => f.debug_struct("ApplyWorkspaceEdit").finish(),
			#[cfg(feature = "lsp")]
			Self::RenamePrepared { token, result, .. } => f.debug_struct("RenamePrepared").field("token", token).field("ok", &result.is_ok()).finish(),
			#[cfg(feature = "lsp")]
			Self::RenameDone { token, result } => f.debug_struct("RenameDone").field("token", token).field("ok", &result.is_ok()).finish(),
		}
	}
}

impl OverlayMsg {
	/// Applies this message to editor state, returning dirty flags.
	pub fn apply(self, editor: &mut Editor) -> Dirty {
		match self {
			Self::Notify(notification) => {
				editor.notify(notification);
				Dirty::REDRAW
			}
			#[cfg(feature = "lsp")]
			Self::ApplyWorkspaceEdit(edit) => {
				editor.enqueue_runtime_workspace_edit_work(edit, None);
				Dirty::REDRAW
			}
			#[cfg(feature = "lsp")]
			Self::RenamePrepared {
				token,
				result,
				encoding,
				expected_prompt,
			} => {
				if editor.state.async_state.pending_rename_token != Some(token) {
					tracing::debug!(token, "Ignoring stale prepare rename result");
					return Dirty::NONE;
				}
				editor.state.async_state.pending_rename_token = None;

				match result {
					Ok(Some(response)) => {
						editor.apply_prepare_rename_response(response, encoding, &expected_prompt);
						Dirty::REDRAW
					}
					Ok(None) => {
						// Server says rename not valid here — close overlay.
						editor.notify(xeno_registry::notifications::keys::warn("Rename not valid at this position"));
						editor.interaction_cancel();
						Dirty::REDRAW
					}
					Err(err) => {
						editor.notify(xeno_registry::notifications::keys::error(err));
						editor.interaction_cancel();
						Dirty::REDRAW
					}
				}
			}
			#[cfg(feature = "lsp")]
			Self::RenameDone { token, result } => {
				if editor.state.async_state.pending_rename_token != Some(token) {
					tracing::debug!(token, "Ignoring stale rename result");
					return Dirty::NONE;
				}
				editor.state.async_state.pending_rename_token = None;

				match result {
					Ok(Some(edit)) => {
						editor.enqueue_runtime_workspace_edit_work(edit, None);
						Dirty::REDRAW
					}
					Ok(None) => {
						editor.notify(xeno_registry::notifications::keys::info("Rename not supported for this buffer"));
						Dirty::REDRAW
					}
					Err(err) => {
						editor.notify(xeno_registry::notifications::keys::error(err));
						Dirty::REDRAW
					}
				}
			}
		}
	}
}

#[cfg(test)]
mod tests;
