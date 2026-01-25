//! Router construction and notification handler registration.

use std::ops::ControlFlow;
use std::sync::Arc;

use tracing::{debug, error, info, warn};

use crate::router::Router;

use super::config::LanguageServerId;
use super::event_handler::{LogLevel, SharedEventHandler};

/// State for the LSP client service.
///
/// This handles incoming server->client notifications and requests.
pub(super) struct ClientState {
	/// Server ID for this client.
	pub server_id: LanguageServerId,
	/// Event handler for LSP events.
	pub event_handler: SharedEventHandler,
}

impl ClientState {
	/// Creates a new client state with the given event handler.
	pub fn new(server_id: LanguageServerId, event_handler: SharedEventHandler) -> Self {
		Self {
			server_id,
			event_handler,
		}
	}
}

/// Build the router for handling server->client messages.
pub(super) fn build_router(state: Arc<ClientState>) -> Router<Arc<ClientState>> {
	let mut router = Router::new(state);
	router
		.notification::<lsp_types::notification::PublishDiagnostics>(|state, params| {
			debug!(
				target: "lsp",
				server_id = state.server_id.0,
				uri = params.uri.as_str(),
				count = params.diagnostics.len(),
				"Received diagnostics"
			);
			state.event_handler.on_diagnostics(
				state.server_id,
				params.uri,
				params.diagnostics,
				params.version,
			);
			ControlFlow::Continue(())
		})
		.notification::<lsp_types::notification::Progress>(|state, params| {
			state.event_handler.on_progress(state.server_id, params);
			ControlFlow::Continue(())
		})
		.notification::<lsp_types::notification::LogMessage>(|state, params| {
			let level = LogLevel::from(params.typ);
			state
				.event_handler
				.on_log_message(state.server_id, level, &params.message);
			match params.typ {
				lsp_types::MessageType::ERROR => {
					error!(target: "lsp", message = %params.message, "Server log")
				}
				lsp_types::MessageType::WARNING => {
					warn!(target: "lsp", message = %params.message, "Server log")
				}
				lsp_types::MessageType::INFO => {
					info!(target: "lsp", message = %params.message, "Server log")
				}
				lsp_types::MessageType::LOG => {
					debug!(target: "lsp", message = %params.message, "Server log")
				}
				_ => {}
			}
			ControlFlow::Continue(())
		})
		.notification::<lsp_types::notification::ShowMessage>(|state, params| {
			let level = LogLevel::from(params.typ);
			state
				.event_handler
				.on_show_message(state.server_id, level, &params.message);
			match params.typ {
				lsp_types::MessageType::ERROR => {
					error!(target: "lsp", message = %params.message, "Server message")
				}
				lsp_types::MessageType::WARNING => {
					warn!(target: "lsp", message = %params.message, "Server message")
				}
				_ => {
					info!(target: "lsp", message = %params.message, "Server message")
				}
			}
			ControlFlow::Continue(())
		})
		.request::<lsp_types::request::WorkspaceConfiguration, _>(|_state, params| {
			let result: Vec<serde_json::Value> =
				params.items.iter().map(|_| serde_json::json!({})).collect();
			async move { Ok(result) }
		})
		.request::<lsp_types::request::WorkDoneProgressCreate, _>(|_state, _params| {
			async move { Ok(()) }
		})
		.unhandled_notification(|_state, notif| {
			debug!(target: "lsp", method = %notif.method, "Unhandled notification");
			ControlFlow::Continue(())
		});
	router
}
