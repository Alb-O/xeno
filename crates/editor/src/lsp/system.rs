//! Editor-side LSP integration root. See `xeno_lsp::session::manager` for full LSP architecture.

#[cfg(feature = "lsp")]
use xeno_lsp::LspManager;

use crate::buffer::Buffer;

#[cfg(feature = "lsp")]
pub struct LspSystem {
	pub(super) inner: RealLspSystem,
}

#[cfg(not(feature = "lsp"))]
pub struct LspSystem;

#[cfg(feature = "lsp")]
#[derive(Clone)]
pub struct LspHandle {
	sync: xeno_lsp::DocumentSync,
}

#[cfg(feature = "lsp")]
impl LspHandle {
	pub async fn close_document(
		&self,
		path: std::path::PathBuf,
		language: String,
	) -> xeno_lsp::Result<()> {
		self.sync.close_document(&path, &language).await
	}
}

#[cfg(feature = "lsp")]
pub(super) struct RealLspSystem {
	manager: LspManager,
	pub(super) sync_manager: crate::lsp::sync_manager::LspSyncManager,
	pub(super) completion: xeno_lsp::CompletionController,
	pub(super) signature_gen: u64,
	pub(super) signature_cancel: Option<tokio_util::sync::CancellationToken>,
	pub(super) ui_tx: tokio::sync::mpsc::UnboundedSender<crate::lsp::LspUiEvent>,
	pub(super) ui_rx: tokio::sync::mpsc::UnboundedReceiver<crate::lsp::LspUiEvent>,
	/// Concrete broker transport handle for shared state requests.
	pub(super) broker: Arc<crate::lsp::broker_transport::BrokerTransport>,
	/// Outbound shared state requests (fire-and-forget from edit path).
	pub(super) shared_state_out_tx:
		tokio::sync::mpsc::UnboundedSender<xeno_broker_proto::types::RequestPayload>,
	/// Inbound shared state events to be drained in editor tick.
	pub(super) shared_state_in_rx:
		tokio::sync::mpsc::UnboundedReceiver<crate::shared_state::SharedStateEvent>,
}

#[cfg(feature = "lsp")]
use std::sync::Arc;

#[cfg(feature = "lsp")]
use xeno_lsp::client::transport::LspTransport;

#[cfg(feature = "lsp")]
impl LspSystem {
	pub fn new() -> Self {
		#[cfg(not(unix))]
		compile_error!("LSP support requires Unix (broker uses Unix domain sockets).");

		let (ui_tx, ui_rx) = tokio::sync::mpsc::unbounded_channel();

		// Keep the concrete BrokerTransport for shared state requests,
		// while passing it as Arc<dyn LspTransport> to the LSP manager.
		let broker = crate::lsp::broker_transport::BrokerTransport::new();
		let transport: Arc<dyn LspTransport> = broker.clone();
		let manager = LspManager::new(transport);
		manager.spawn_router();

		// Outbound: edit path enqueues sync work here (fire-and-forget).
		let (shared_state_out_tx, mut shared_state_out_rx) =
			tokio::sync::mpsc::unbounded_channel::<xeno_broker_proto::types::RequestPayload>();

		// Inbound: events/results delivered to editor tick for processing.
		let (shared_state_in_tx, shared_state_in_rx) =
			tokio::sync::mpsc::unbounded_channel::<crate::shared_state::SharedStateEvent>();

		// Shared state tasks require a Tokio runtime; skip in unit tests.
		if tokio::runtime::Handle::try_current().is_ok() {
			// Task A: forward broker transport async events → shared_state_in_tx.
			if let Some(mut event_rx) = broker.take_shared_state_events() {
				let in_tx_a = shared_state_in_tx.clone();
				tokio::spawn(async move {
					while let Some(evt) = event_rx.recv().await {
						if in_tx_a.send(evt).is_err() {
							break;
						}
					}
					let _ = in_tx_a.send(crate::shared_state::SharedStateEvent::Disconnected);
				});
			}

			// Task B: outbound sender — drains editor requests, calls broker, posts results back.
			let broker_b = broker.clone();
			let in_tx_b = shared_state_in_tx;
			tokio::spawn(async move {
				use xeno_broker_proto::types::{RequestPayload, ResponsePayload};

				while let Some(payload) = shared_state_out_rx.recv().await {
					let is_apply = matches!(payload, RequestPayload::SharedApply { .. });

					// Extract URI for error reporting.
					let uri = match &payload {
						RequestPayload::SharedOpen { uri, .. }
						| RequestPayload::SharedClose { uri }
						| RequestPayload::SharedApply { uri, .. }
						| RequestPayload::SharedActivity { uri }
						| RequestPayload::SharedFocus { uri, .. }
						| RequestPayload::SharedResync { uri, .. } => uri.clone(),
						_ => continue,
					};

					match broker_b.shared_state_request_raw(payload).await {
						Ok(resp) => {
							// Convert response into an inbound event for the editor.
							let evt = match resp {
								ResponsePayload::SharedOpened { snapshot, text } => {
									Some(crate::shared_state::SharedStateEvent::Opened {
										snapshot,
										text,
									})
								}
								ResponsePayload::SharedApplyAck {
									uri,
									kind,
									epoch,
									seq,
									applied_tx,
									hash64,
									len_chars,
									history_from_id,
									history_to_id,
									history_group,
								} => Some(crate::shared_state::SharedStateEvent::ApplyAck {
									uri,
									kind,
									epoch,
									seq,
									applied_tx,
									hash64,
									len_chars,
									history_from_id,
									history_to_id,
									history_group,
								}),
								ResponsePayload::SharedSnapshot {
									nonce,
									text,
									snapshot,
								} => Some(crate::shared_state::SharedStateEvent::Snapshot {
									uri,
									nonce,
									text,
									snapshot,
								}),
								ResponsePayload::SharedFocusAck {
									nonce,
									snapshot,
									repair_text,
								} => Some(crate::shared_state::SharedStateEvent::FocusAck {
									nonce,
									snapshot,
									repair_text,
								}),
								ResponsePayload::SharedActivityAck => None,
								ResponsePayload::SharedClosed => None,
								_ => None,
							};
							if let Some(evt) = evt {
								let _ = in_tx_b.send(evt);
							}
						}
						Err(e) => {
							tracing::warn!(?uri, error = ?e, "shared state request failed");
							if is_apply {
								let evt = map_shared_state_apply_error(uri, e);
								let _ = in_tx_b.send(evt);
							} else {
								let _ = in_tx_b.send(
									crate::shared_state::SharedStateEvent::RequestFailed { uri },
								);
							}
						}
					}
				}
			});
		}

		Self {
			inner: RealLspSystem {
				manager,
				sync_manager: crate::lsp::sync_manager::LspSyncManager::new(),
				completion: xeno_lsp::CompletionController::new(),
				signature_gen: 0,
				signature_cancel: None,
				ui_tx,
				ui_rx,
				broker,
				shared_state_out_tx,
				shared_state_in_rx,
			},
		}
	}

	pub fn handle(&self) -> LspHandle {
		LspHandle {
			sync: self.inner.manager.sync().clone(),
		}
	}
}

#[cfg(feature = "lsp")]
fn map_shared_state_apply_error(
	uri: String,
	err: xeno_broker_proto::types::ErrorCode,
) -> crate::shared_state::SharedStateEvent {
	use xeno_broker_proto::types::ErrorCode;

	match err {
		ErrorCode::NothingToUndo => crate::shared_state::SharedStateEvent::NothingToUndo { uri },
		ErrorCode::NothingToRedo => crate::shared_state::SharedStateEvent::NothingToRedo { uri },
		ErrorCode::HistoryUnavailable | ErrorCode::NotImplemented => {
			crate::shared_state::SharedStateEvent::HistoryUnavailable { uri }
		}
		_ => crate::shared_state::SharedStateEvent::EditRejected { uri },
	}
}

#[cfg(all(test, feature = "lsp"))]
mod tests {
	use xeno_broker_proto::types::ErrorCode;

	use super::map_shared_state_apply_error;
	use crate::shared_state::SharedStateEvent;

	#[test]
	fn history_unavailable_maps_to_history_event() {
		let evt =
			map_shared_state_apply_error("file:///test".to_string(), ErrorCode::HistoryUnavailable);
		assert!(
			matches!(evt, SharedStateEvent::HistoryUnavailable { .. }),
			"HistoryUnavailable should map to SharedStateEvent::HistoryUnavailable"
		);
	}
}

#[cfg(not(feature = "lsp"))]
impl LspSystem {
	pub fn new() -> Self {
		Self
	}
}

impl Default for LspSystem {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(feature = "lsp")]
impl LspSystem {
	pub fn poll_diagnostics(&mut self) -> Vec<xeno_lsp::DiagnosticsEvent> {
		self.inner.manager.poll_diagnostics()
	}

	pub fn diagnostics_version(&self) -> u64 {
		self.inner.manager.diagnostics_version()
	}

	pub fn configure_server(
		&self,
		language: impl Into<String>,
		config: xeno_lsp::LanguageServerConfig,
	) {
		self.inner.manager.configure_server(language, config);
	}

	pub fn remove_server(&self, language: &str) {
		self.inner.manager.remove_server(language);
	}

	pub fn sync(&self) -> &xeno_lsp::DocumentSync {
		self.inner.manager.sync()
	}

	pub fn sync_clone(&self) -> xeno_lsp::DocumentSync {
		self.inner.manager.sync().clone()
	}

	pub fn registry(&self) -> &xeno_lsp::Registry {
		self.inner.manager.registry()
	}

	pub fn documents(&self) -> &xeno_lsp::DocumentStateManager {
		self.inner.manager.documents()
	}

	pub fn get_diagnostics(&self, buffer: &Buffer) -> Vec<xeno_lsp::lsp_types::Diagnostic> {
		buffer
			.path()
			.as_ref()
			.map(|p| self.sync().get_diagnostics(p))
			.unwrap_or_default()
	}

	pub fn error_count(&self, buffer: &Buffer) -> usize {
		buffer
			.path()
			.as_ref()
			.map(|p| self.sync().error_count(p))
			.unwrap_or(0)
	}

	pub fn warning_count(&self, buffer: &Buffer) -> usize {
		buffer
			.path()
			.as_ref()
			.map(|p| self.sync().warning_count(p))
			.unwrap_or(0)
	}

	pub fn total_error_count(&self) -> usize {
		self.inner.manager.sync().total_error_count()
	}

	pub fn total_warning_count(&self) -> usize {
		self.inner.manager.sync().total_warning_count()
	}

	pub async fn shutdown_all(&self) {
		self.inner.manager.shutdown_all().await;
	}

	pub(crate) fn sync_manager(&self) -> &crate::lsp::sync_manager::LspSyncManager {
		&self.inner.sync_manager
	}

	pub(crate) fn sync_manager_mut(&mut self) -> &mut crate::lsp::sync_manager::LspSyncManager {
		&mut self.inner.sync_manager
	}
}
