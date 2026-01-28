use std::future::Future;
use std::pin::Pin;

use xeno_primitives::Selection;
use xeno_primitives::range::CharIdx;
use xeno_registry::notifications::keys;

use crate::buffer::ViewId;
use crate::impls::Editor;
use crate::overlay::{CloseReason, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy};
use crate::window::GutterSelector;

pub struct RenameOverlay {
	target: ViewId,
	position: CharIdx,
	initial_word: String,
}

impl RenameOverlay {
	pub fn new(target: ViewId, position: CharIdx, initial_word: String) -> Self {
		Self {
			target,
			position,
			initial_word,
		}
	}
}

impl OverlayController for RenameOverlay {
	fn name(&self) -> &'static str {
		"Rename"
	}

	fn ui_spec(&self, _ed: &Editor) -> OverlayUiSpec {
		OverlayUiSpec {
			title: Some("Rename".into()),
			gutter: GutterSelector::Prompt('>'),
			rect: RectPolicy::TopCenter {
				width_percent: 60,
				max_width: 80,
				min_width: 40,
				y_frac: (1, 5),
				height: 3,
			},
			style: crate::overlay::prompt_style("Rename"),
			windows: vec![],
		}
	}

	fn on_open(&mut self, ed: &mut Editor, session: &mut OverlaySession) {
		if !self.initial_word.is_empty() {
			let end = self.initial_word.chars().count();
			if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(session.input) {
				buffer.reset_content(self.initial_word.as_str());
				buffer.set_cursor_and_selection(end, Selection::single(0, end));
			}
		}
	}

	fn on_input_changed(&mut self, _ed: &mut Editor, _session: &mut OverlaySession, _text: &str) {}

	fn on_commit<'a>(
		&'a mut self,
		ed: &'a mut Editor,
		session: &'a mut OverlaySession,
	) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let new_name = session
			.input_text(ed)
			.trim_end_matches('\n')
			.trim()
			.to_string();
		let target_buffer = self.target;
		let position = self.position;

		Box::pin(async move {
			if new_name.is_empty() {
				return;
			}
			apply_rename(ed, target_buffer, position, new_name).await;
		})
	}

	fn on_close(&mut self, _ed: &mut Editor, _session: &mut OverlaySession, _reason: CloseReason) {}
}

async fn apply_rename(ed: &mut Editor, buffer_id: ViewId, position: usize, new_name: String) {
	#[cfg(feature = "lsp")]
	{
		let Some(buffer) = ed.state.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		if buffer.is_readonly() {
			ed.notify(keys::BUFFER_READONLY);
			return;
		}
		let Some((client, uri, _)) = ed.state.lsp.prepare_position_request(buffer).ok().flatten()
		else {
			ed.notify(keys::warn("Rename not supported for this buffer"));
			return;
		};
		let encoding = client.offset_encoding();
		let Some(pos) = buffer
			.with_doc(|doc| xeno_lsp::char_to_lsp_position(doc.content(), position, encoding))
		else {
			ed.notify(keys::error("Invalid rename position"));
			return;
		};

		match client.rename(uri, pos, new_name).await {
			Ok(Some(edit)) => {
				if let Err(err) = ed.apply_workspace_edit(edit).await {
					ed.notify(keys::error(err.to_string()));
				}
			}
			Ok(None) => {
				ed.notify(keys::info("Rename not supported for this buffer"));
			}
			Err(err) => {
				ed.notify(keys::error(err.to_string()));
			}
		}
	}
	#[cfg(not(feature = "lsp"))]
	{
		let _ = (buffer_id, position, new_name);
		ed.notify(keys::warn("LSP not enabled"));
	}
}
