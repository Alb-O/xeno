//! LSP completion controller with debounce and cancellation.

use std::time::Duration;

use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use xeno_base::range::CharIdx;
use xeno_lsp::ClientHandle;
use xeno_lsp::lsp_types::{CompletionContext, CompletionTriggerKind, Position, Uri};

use crate::buffer::BufferId;
use crate::editor::lsp_events::LspUiEvent;

pub enum CompletionTrigger {
	Typing,
	Manual,
}

pub struct CompletionController {
	generation: u64,
	in_flight: Option<InFlightCompletion>,
}

struct InFlightCompletion {
	cancel: CancellationToken,
}

impl CompletionController {
	pub fn new() -> Self {
		Self {
			generation: 0,
			in_flight: None,
		}
	}

	pub fn generation(&self) -> u64 {
		self.generation
	}

	pub fn cancel(&mut self) {
		if let Some(in_flight) = self.in_flight.take() {
			in_flight.cancel.cancel();
		}
	}

	pub fn trigger(&mut self, request: CompletionRequest) {
		self.generation = self.generation.wrapping_add(1);
		let generation = self.generation;
		if let Some(in_flight) = self.in_flight.take() {
			in_flight.cancel.cancel();
		}

		let cancel = CancellationToken::new();
		self.in_flight = Some(InFlightCompletion {
			cancel: cancel.clone(),
		});

		tokio::spawn(async move {
			if request.debounce > Duration::ZERO {
				tokio::select! {
					_ = cancel.cancelled() => return,
					_ = sleep(request.debounce) => {}
				}
			} else if cancel.is_cancelled() {
				return;
			}

			let response = request
				.client
				.completion(
					request.uri,
					request.position,
					Some(CompletionContext {
						trigger_kind: request.trigger_kind,
						trigger_character: request.trigger_character.clone(),
					}),
				)
				.await
				.ok()
				.flatten();

			if cancel.is_cancelled() {
				return;
			}

			let _ = request.ui_tx.send(LspUiEvent::CompletionResult {
				generation,
				buffer_id: request.buffer_id,
				cursor: request.cursor,
				doc_version: request.doc_version,
				replace_start: request.replace_start,
				response,
			});
		});
	}
}

pub struct CompletionRequest {
	pub buffer_id: BufferId,
	pub cursor: CharIdx,
	pub doc_version: u64,
	pub replace_start: usize,
	pub client: ClientHandle,
	pub uri: Uri,
	pub position: Position,
	pub debounce: Duration,
	pub ui_tx: tokio::sync::mpsc::UnboundedSender<LspUiEvent>,
	pub trigger_kind: CompletionTriggerKind,
	pub trigger_character: Option<String>,
}

impl CompletionTrigger {
	pub fn debounce(&self) -> Duration {
		match self {
			CompletionTrigger::Typing => Duration::from_millis(80),
			CompletionTrigger::Manual => Duration::ZERO,
		}
	}
}
