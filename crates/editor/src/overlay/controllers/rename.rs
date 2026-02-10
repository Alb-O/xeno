use std::future::Future;
use std::pin::Pin;

use xeno_primitives::Selection;
use xeno_primitives::range::CharIdx;
use xeno_registry::notifications::keys;

use crate::buffer::ViewId;
#[cfg(feature = "lsp")]
use crate::msg::OverlayMsg;
use crate::overlay::{
	CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy,
};
use crate::window::GutterSelector;

#[cfg_attr(not(feature = "lsp"), allow(dead_code))]
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

	fn ui_spec(&self, _ctx: &dyn OverlayContext) -> OverlayUiSpec {
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

	fn on_open(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) {
		if !self.initial_word.is_empty() {
			let end = self.initial_word.chars().count();
			ctx.reset_buffer_content(session.input, self.initial_word.as_str());
			if let Some(buffer) = ctx.buffer_mut(session.input) {
				buffer.set_cursor_and_selection(end, Selection::single(0, end));
			}
		}
	}

	fn on_input_changed(
		&mut self,
		_ctx: &mut dyn OverlayContext,
		_session: &mut OverlaySession,
		_text: &str,
	) {
	}

	fn on_commit<'a>(
		&'a mut self,
		ctx: &'a mut dyn OverlayContext,
		session: &'a mut OverlaySession,
	) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let new_name = session
			.input_text(ctx)
			.trim_end_matches('\n')
			.trim()
			.to_string();
		if new_name.is_empty() {
			return Box::pin(async {});
		}

		#[cfg(feature = "lsp")]
		{
			let Some(buffer) = ctx.buffer(self.target) else {
				return Box::pin(async {});
			};
			if buffer.is_readonly() {
				ctx.notify(keys::BUFFER_READONLY.into());
				return Box::pin(async {});
			}

			let Some((client, uri, _)) = ctx.lsp_prepare_position_request(buffer).ok().flatten()
			else {
				ctx.notify(keys::warn("Rename not supported for this buffer"));
				return Box::pin(async {});
			};

			let encoding = client.offset_encoding();
			let Some(pos) = buffer.with_doc(|doc| {
				xeno_lsp::char_to_lsp_position(doc.content(), self.position, encoding)
			}) else {
				ctx.notify(keys::error("Invalid rename position"));
				return Box::pin(async {});
			};

			let tx = ctx.msg_tx();
			tokio::spawn(async move {
				let msg = match client.rename(uri, pos, new_name).await {
					Ok(Some(edit)) => OverlayMsg::ApplyWorkspaceEdit(edit),
					Ok(None) => {
						OverlayMsg::Notify(keys::info("Rename not supported for this buffer"))
					}
					Err(err) => OverlayMsg::Notify(keys::error(err.to_string())),
				};

				let _ = tx.send(msg.into());
			});

			return Box::pin(async {});
		}

		#[cfg(not(feature = "lsp"))]
		{
			ctx.notify(keys::warn("LSP not enabled"));
			Box::pin(async {})
		}
	}

	fn on_close(
		&mut self,
		_ctx: &mut dyn OverlayContext,
		_session: &mut OverlaySession,
		_reason: CloseReason,
	) {
	}
}
