use crate::buffer::ViewId;
use crate::impls::Editor;
use crate::overlay::{CloseReason, controllers};

impl Editor {
	pub fn interaction_on_buffer_edited(&mut self) {
		let view_id = self.focused_view();
		let mut interaction: crate::overlay::OverlayManager =
			std::mem::take(&mut self.state.overlay_system.interaction);
		interaction.on_buffer_edited(self, view_id);
		self.state.overlay_system.interaction = interaction;
		self.flush_effects();
	}

	pub async fn interaction_commit(&mut self) {
		let mut interaction: crate::overlay::OverlayManager =
			std::mem::take(&mut self.state.overlay_system.interaction);
		interaction.commit(self).await;
		self.state.overlay_system.interaction = interaction;
		self.flush_effects();
	}

	/// Completes a deferred overlay commit if one is pending.
	///
	/// [`OverlayController::on_commit`] is async, so `CloseModal { Commit }`
	/// effects set [`FrameState::pending_overlay_commit`] instead of running
	/// the commit inline. This method MUST be called from every async
	/// continuation that follows a [`flush_effects`](Self::flush_effects) call.
	pub async fn flush_pending_overlay_commit(&mut self) {
		if self.state.frame.pending_overlay_commit {
			self.state.frame.pending_overlay_commit = false;
			self.interaction_commit().await;
		}
	}

	pub fn interaction_cancel(&mut self) {
		let mut interaction: crate::overlay::OverlayManager =
			std::mem::take(&mut self.state.overlay_system.interaction);
		interaction.close(self, CloseReason::Cancel);
		self.state.overlay_system.interaction = interaction;
		self.flush_effects();
	}

	pub fn open_search(&mut self, reverse: bool) -> bool {
		let ctl = controllers::SearchOverlay::new(self.focused_view(), reverse);
		let mut interaction: crate::overlay::OverlayManager =
			std::mem::take(&mut self.state.overlay_system.interaction);
		let result = interaction.open(self, Box::new(ctl));
		self.state.overlay_system.interaction = interaction;
		self.flush_effects();
		result
	}

	pub fn open_command_palette(&mut self) -> bool {
		let ctl = controllers::CommandPaletteOverlay::new();
		let mut interaction: crate::overlay::OverlayManager =
			std::mem::take(&mut self.state.overlay_system.interaction);
		let result = interaction.open(self, Box::new(ctl));
		self.state.overlay_system.interaction = interaction;
		self.flush_effects();
		result
	}

	pub fn open_workspace_search(&mut self) -> bool {
		let ctl = controllers::WorkspaceSearchOverlay::new();
		let mut interaction: crate::overlay::OverlayManager =
			std::mem::take(&mut self.state.overlay_system.interaction);
		let result = interaction.open(self, Box::new(ctl));
		self.state.overlay_system.interaction = interaction;
		self.flush_effects();
		result
	}

	pub fn open_rename(&mut self) -> bool {
		let buffer_id = self.focused_view();
		let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
			return false;
		};

		#[cfg(feature = "lsp")]
		let rename_supported = self
			.state
			.lsp
			.prepare_position_request(buffer)
			.ok()
			.flatten()
			.is_some_and(|(client, _, _)| client.supports_rename());
		#[cfg(not(feature = "lsp"))]
		let rename_supported = false;

		if !rename_supported {
			self.notify(xeno_registry::notifications::keys::warn(
				"Rename not supported for this buffer",
			));
			self.flush_effects();
			return false;
		}

		let cursor = buffer.cursor;
		let word = word_at_cursor(buffer);

		let ctl = controllers::RenameOverlay::new(buffer_id, cursor, word);
		let mut interaction: crate::overlay::OverlayManager =
			std::mem::take(&mut self.state.overlay_system.interaction);
		let result = interaction.open(self, Box::new(ctl));
		self.state.overlay_system.interaction = interaction;
		self.flush_effects();
		result
	}

	/// Broadcasts an event to all passive overlay layers.
	pub fn notify_overlay_event(&mut self, event: crate::overlay::LayerEvent) {
		self.state.effects.push_layer_event(event);
		self.flush_effects();
	}

	/// Ensures the cursor is visible in the specified view, scrolling if necessary.
	///
	/// Synchronizes the viewport visibility logic with the render pipeline by
	/// using the same gutter layout and text width calculation.
	pub fn reveal_cursor_in_view(&mut self, buffer_id: ViewId) {
		use xeno_registry::options::keys as opt_keys;
		let tab_width = self.resolve_typed_option(buffer_id, opt_keys::TAB_WIDTH) as usize;
		let scroll_margin = self.resolve_typed_option(buffer_id, opt_keys::SCROLL_MARGIN) as usize;
		let area = self.view_area(buffer_id);

		if let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) {
			let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
			let is_diff_file = buffer.file_type().is_some_and(|ft| ft == "diff");
			let gutter = crate::window::GutterSelector::Registry;
			let effective_gutter = if is_diff_file {
				crate::render::BufferRenderContext::diff_gutter_selector(gutter)
			} else {
				gutter
			};

			let gutter_layout = crate::render::GutterLayout::from_selector(
				effective_gutter,
				total_lines,
				area.width,
			);
			let text_width = area.width.saturating_sub(gutter_layout.total_width) as usize;

			crate::render::ensure_buffer_cursor_visible(
				buffer,
				area,
				text_width,
				tab_width,
				scroll_margin,
			);
			self.state.effects.request_redraw();
		}
		self.flush_effects();
	}
}

fn word_at_cursor(buffer: &crate::buffer::Buffer) -> String {
	buffer.with_doc(|doc| {
		let content = doc.content();
		let mut start = buffer.cursor.min(content.len_chars());
		let mut end = start;
		while start > 0 {
			let ch = content.char(start - 1);
			if is_word_char(ch) {
				start = start.saturating_sub(1);
			} else {
				break;
			}
		}
		while end < content.len_chars() {
			let ch = content.char(end);
			if is_word_char(ch) {
				end = end.saturating_add(1);
			} else {
				break;
			}
		}
		if start >= end {
			return String::new();
		}
		content.slice(start..end).to_string()
	})
}

fn is_word_char(ch: char) -> bool {
	ch.is_alphanumeric() || ch == '_'
}
