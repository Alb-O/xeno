//! LSP signature help triggering and display.
//!
//! Signature help shows function/method signatures while the user types arguments,
//! typically triggered by `(` or `,`. The language server returns the full signature
//! along with documentation; this module formats that into a popup displayed near
//! the cursor.
//!
//! Requests are cancellable—if the user continues typing before results arrive,
//! stale responses are discarded.

use tokio_util::sync::CancellationToken;
use xeno_lsp::lsp_types::{Documentation, MarkupContent, SignatureHelp};

use crate::buffer::ViewId;
use crate::impls::Editor;
use crate::info_popup::PopupAnchor;

impl Editor {
	pub(crate) fn trigger_signature_help(&mut self) {
		let buffer_id = self.focused_view();
		let (client, uri, position, cursor, doc_version) = {
			let buffer = self.buffer();
			if buffer.mode() != xeno_primitives::Mode::Insert {
				return;
			}
			let Some((client, uri, position)) = self
				.state
				.lsp
				.prepare_position_request(buffer)
				.ok()
				.flatten()
			else {
				return;
			};
			if !client.supports_signature_help() {
				return;
			}
			(client, uri, position, buffer.cursor, buffer.version())
		};

		self.cancel_signature_help();
		self.state.signature_help_generation = self.state.signature_help_generation.wrapping_add(1);
		let generation = self.state.signature_help_generation;

		let cancel = CancellationToken::new();
		self.state.signature_help_cancel = Some(cancel.clone());

		let anchor = signature_help_anchor(self, buffer_id);
		let ui_tx = self.state.lsp_ui_tx.clone();

		tokio::spawn(async move {
			let help = tokio::select! {
				_ = cancel.cancelled() => return,
				result = client.signature_help(uri, position) => result,
			};

			if cancel.is_cancelled() {
				return;
			}

			let help = match help {
				Ok(Some(help)) => help,
				_ => return,
			};

			let contents = format_signature_help(&help);
			if contents.is_empty() {
				return;
			}

			let _ = ui_tx.send(super::events::LspUiEvent::SignatureHelp {
				generation,
				buffer_id,
				cursor,
				doc_version,
				contents,
				anchor,
			});
		});
	}

	pub(crate) fn cancel_signature_help(&mut self) {
		if let Some(cancel) = self.state.signature_help_cancel.take() {
			cancel.cancel();
		}
	}
}

fn signature_help_anchor(editor: &Editor, buffer_id: ViewId) -> PopupAnchor {
	let Some(buffer) = editor.get_buffer(buffer_id) else {
		return PopupAnchor::Center;
	};
	let tab_width = editor.tab_width_for(buffer_id);
	let Some((row, col)) = buffer.doc_to_screen_position(buffer.cursor, tab_width) else {
		return PopupAnchor::Center;
	};
	let view_area = editor.focused_view_area();
	let x = view_area.x.saturating_add(col);
	let y = view_area.y.saturating_add(row.saturating_add(1));
	PopupAnchor::Point { x, y }
}

fn format_signature_help(help: &SignatureHelp) -> String {
	let signature = help
		.active_signature
		.and_then(|idx| help.signatures.get(idx as usize))
		.or_else(|| help.signatures.first());
	let Some(signature) = signature else {
		return String::new();
	};

	let mut output = signature.label.clone();
	if let Some(doc) = signature.documentation.as_ref() {
		let doc = format_documentation(doc);
		if !doc.is_empty() {
			output.push_str("\n\n");
			output.push_str(&doc);
		}
	}

	output
}

fn format_documentation(doc: &Documentation) -> String {
	match doc {
		Documentation::String(text) => text.clone(),
		Documentation::MarkupContent(MarkupContent { value, .. }) => value.clone(),
	}
}
