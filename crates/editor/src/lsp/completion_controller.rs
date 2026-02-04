//! Completion and signature help controller methods.

#[cfg(feature = "lsp")]
use super::system::LspSystem;

#[cfg(feature = "lsp")]
impl LspSystem {
	pub(crate) fn completion_generation(&self) -> u64 {
		self.inner.completion.generation()
	}

	pub(crate) fn trigger_completion(
		&mut self,
		request: xeno_lsp::CompletionRequest<crate::buffer::ViewId>,
	) {
		use crate::lsp::LspUiEvent;
		let ui_tx = self.inner.ui_tx.clone();
		self.inner.completion.trigger(
			request,
			move |generation, buffer_id, replace_start, response| {
				let _ = ui_tx.send(LspUiEvent::CompletionResult {
					generation,
					buffer_id,
					replace_start,
					response,
				});
			},
		);
	}

	pub(crate) fn cancel_completion(&mut self) {
		self.inner.completion.cancel();
	}

	pub(crate) fn signature_help_generation(&self) -> u64 {
		self.inner.signature_gen
	}

	pub(crate) fn bump_signature_help_generation(&mut self) -> u64 {
		self.inner.signature_gen = self.inner.signature_gen.wrapping_add(1);
		self.inner.signature_gen
	}

	pub(crate) fn set_signature_help_cancel(
		&mut self,
		cancel: tokio_util::sync::CancellationToken,
	) {
		self.inner.signature_cancel = Some(cancel);
	}

	pub(crate) fn take_signature_help_cancel(
		&mut self,
	) -> Option<tokio_util::sync::CancellationToken> {
		self.inner.signature_cancel.take()
	}

	pub(crate) fn ui_tx(&self) -> tokio::sync::mpsc::UnboundedSender<crate::lsp::LspUiEvent> {
		self.inner.ui_tx.clone()
	}

	pub(crate) fn try_recv_ui_event(&mut self) -> Option<crate::lsp::LspUiEvent> {
		self.inner.ui_rx.try_recv().ok()
	}
}
