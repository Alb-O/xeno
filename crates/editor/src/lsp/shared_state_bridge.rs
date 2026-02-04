//! Shared state broker bridge accessors.

#[cfg(feature = "lsp")]
use std::sync::Arc;

#[cfg(feature = "lsp")]
use super::system::LspSystem;

#[cfg(feature = "lsp")]
impl LspSystem {
	/// Returns a sender for fire-and-forget shared state outbound requests.
	pub(crate) fn shared_state_out_tx(
		&self,
	) -> &tokio::sync::mpsc::UnboundedSender<xeno_broker_proto::types::RequestPayload> {
		&self.inner.shared_state_out_tx
	}

	/// Try to receive the next inbound shared state event.
	pub(crate) fn try_recv_shared_state_in(
		&mut self,
	) -> Option<crate::shared_state::SharedStateEvent> {
		self.inner.shared_state_in_rx.try_recv().ok()
	}

	/// Returns the broker session ID for this editor.
	pub(crate) fn broker_session_id(&self) -> xeno_broker_proto::types::SessionId {
		self.inner.broker.session_id()
	}

	/// Returns a clone of the broker transport handle.
	pub(crate) fn broker_transport(&self) -> Arc<crate::lsp::broker_transport::BrokerTransport> {
		self.inner.broker.clone()
	}
}
