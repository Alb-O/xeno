use crate::buffer::ViewId;
use crate::impls::Editor;
#[cfg(feature = "lsp")]
use crate::overlay::OverlayContext;
use crate::overlay::{CloseReason, controllers};

impl Editor {
	pub fn interaction_on_buffer_edited(&mut self) {
		let view_id = self.focused_view();
		let mut interaction = self.state.ui.overlay_system.take_interaction();
		interaction.on_buffer_edited(self, view_id);
		self.state.ui.overlay_system.restore_interaction(interaction);
		self.flush_effects();
	}

	pub async fn interaction_commit(&mut self) {
		let mut interaction = self.state.ui.overlay_system.take_interaction();
		interaction.commit(self).await;
		self.state.ui.overlay_system.restore_interaction(interaction);
		self.flush_effects();
	}

	pub fn interaction_cancel(&mut self) {
		let mut interaction = self.state.ui.overlay_system.take_interaction();
		interaction.close(self, CloseReason::Cancel);
		self.state.ui.overlay_system.restore_interaction(interaction);
		self.flush_effects();
	}

	pub fn open_search(&mut self, reverse: bool) -> bool {
		let ctl = controllers::SearchOverlay::new(self.focused_view(), reverse);
		let mut interaction = self.state.ui.overlay_system.take_interaction();
		let result = interaction.open(self, Box::new(ctl));
		self.state.ui.overlay_system.restore_interaction(interaction);
		self.flush_effects();
		result
	}

	pub fn open_command_palette(&mut self) -> bool {
		let ctl = controllers::CommandPaletteOverlay::new();
		let mut interaction = self.state.ui.overlay_system.take_interaction();
		let result = interaction.open(self, Box::new(ctl));
		self.state.ui.overlay_system.restore_interaction(interaction);
		self.flush_effects();
		result
	}

	pub fn open_file_picker(&mut self) -> bool {
		let ctl = controllers::FilePickerOverlay::new(None);
		let mut interaction = self.state.ui.overlay_system.take_interaction();
		let result = interaction.open(self, Box::new(ctl));
		self.state.ui.overlay_system.restore_interaction(interaction);
		self.flush_effects();
		result
	}

	pub fn open_workspace_search(&mut self) -> bool {
		let ctl = controllers::WorkspaceSearchOverlay::new();
		let mut interaction = self.state.ui.overlay_system.take_interaction();
		let result = interaction.open(self, Box::new(ctl));
		self.state.ui.overlay_system.restore_interaction(interaction);
		self.flush_effects();
		result
	}

	pub fn open_rename(&mut self) -> bool {
		let buffer_id = self.focused_view();
		let Some(buffer) = self.state.core.editor.buffers.get_buffer(buffer_id) else {
			return false;
		};

		#[cfg(feature = "lsp")]
		let rename_supported = self
			.state
			.integration
			.lsp
			.prepare_position_request(buffer)
			.ok()
			.flatten()
			.is_some_and(|(client, _, _)| client.supports_rename());
		#[cfg(not(feature = "lsp"))]
		let rename_supported = false;

		if !rename_supported {
			self.notify(xeno_registry::notifications::keys::warn("Rename not supported for this buffer"));
			self.flush_effects();
			return false;
		}

		let cursor = buffer.cursor;
		let word = word_at_cursor(buffer);

		let ctl = controllers::RenameOverlay::new(buffer_id, cursor, word.clone());
		let mut interaction = self.state.ui.overlay_system.take_interaction();
		let result = interaction.open(self, Box::new(ctl));
		self.state.ui.overlay_system.restore_interaction(interaction);
		self.flush_effects();

		// Spawn prepareRename in background if server supports it, to validate
		// the rename position and get an authoritative range/placeholder.
		#[cfg(feature = "lsp")]
		if result {
			// Re-borrow buffer after flush_effects released the earlier borrow.
			let buffer = self.state.core.editor.buffers.get_buffer(buffer_id).unwrap();
			if let Some((client, uri, _)) = self.state.integration.lsp.prepare_position_request(buffer).ok().flatten()
				&& client.supports_prepare_rename()
			{
				let encoding = client.offset_encoding();
				let pos = buffer.with_doc(|doc| xeno_lsp::char_to_lsp_position(doc.content(), cursor, encoding));
				if let Some(pos) = pos {
					let token = self.mint_rename_token();
					let tx = self.msg_tx();
					let expected_prompt = word;
					xeno_worker::spawn(xeno_worker::TaskClass::Background, async move {
						let result = client.prepare_rename(uri, pos).await.map_err(|e| e.to_string());
						let _ = tx.send(
							crate::msg::OverlayMsg::RenamePrepared {
								token,
								result,
								encoding,
								expected_prompt,
							}
							.into(),
						);
					});
				}
			}
		}

		result
	}

	/// Applies a successful prepareRename response to the active rename overlay.
	///
	/// If the response includes a placeholder, updates the overlay prompt text
	/// (only if the user hasn't edited it yet). If the response includes a range,
	/// highlights the target symbol in the source buffer.
	#[cfg(feature = "lsp")]
	pub(crate) fn apply_prepare_rename_response(
		&mut self,
		response: xeno_lsp::lsp_types::PrepareRenameResponse,
		encoding: xeno_lsp::OffsetEncoding,
		expected_prompt: &str,
	) {
		use xeno_lsp::lsp_types::PrepareRenameResponse;

		let (placeholder, range) = match &response {
			PrepareRenameResponse::Range(range) => (None, Some(*range)),
			PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } => (Some(placeholder.as_str()), Some(*range)),
			PrepareRenameResponse::DefaultBehavior { .. } => (None, None),
		};

		// Update the overlay prompt text with placeholder if user hasn't edited.
		if let Some(placeholder) = placeholder {
			let active = self.state.ui.overlay_system.interaction().active();
			if let Some(active) = active {
				let input_id = active.session.input;
				// Check if prompt still matches the initial word (user hasn't edited).
				let current_text = self
					.state
					.core
					.editor
					.buffers
					.get_buffer(input_id)
					.map(|b| b.with_doc(|doc| doc.content().to_string()))
					.unwrap_or_default();
				let current_trimmed = current_text.trim_end_matches('\n');
				// Only replace if the user hasn't edited the prompt yet.
				if current_trimmed == expected_prompt {
					let end = placeholder.chars().count();
					if let Some(buffer) = self.state.core.editor.buffers.get_buffer_mut(input_id) {
						buffer.reset_content(placeholder);
						buffer.set_cursor_and_selection(end, xeno_primitives::Selection::single(0, end));
					}
				}
			}
		}

		// Highlight the target range in the source buffer.
		if let Some(range) = range {
			let active = self.state.ui.overlay_system.interaction().active();
			if let Some(active) = active
				&& active.controller.kind() == crate::overlay::OverlayControllerKind::Rename
			{
				// Find the target buffer (first non-input pane).
				if let Some(target_view) = active
					.session
					.panes
					.iter()
					.find_map(|p| if p.buffer != active.session.input { Some(p.buffer) } else { None })
					&& let Some(buffer) = self.state.core.editor.buffers.get_buffer_mut(target_view)
				{
					let start_char = buffer.with_doc(|doc| xeno_lsp::lsp_position_to_char(doc.content(), range.start, encoding));
					let end_char = buffer.with_doc(|doc| xeno_lsp::lsp_position_to_char(doc.content(), range.end, encoding));
					if let (Some(start), Some(end)) = (start_char, end_char) {
						buffer.set_selection(xeno_primitives::Selection::single(start, end));
					}
				}
			}
		}

		self.state.core.frame.needs_redraw = true;
	}

	/// Broadcasts an event to all passive overlay layers.
	pub fn notify_overlay_event(&mut self, event: crate::overlay::LayerEvent) {
		self.state.runtime.effects.push_layer_event(event);
		self.flush_effects();
	}

	pub fn interaction_refresh_file_picker(&mut self) {
		let mut interaction = self.state.ui.overlay_system.take_interaction();
		interaction.refresh_if_kind(self, crate::overlay::OverlayControllerKind::FilePicker);
		self.state.ui.overlay_system.restore_interaction(interaction);
	}

	/// Ensures the cursor is visible in the specified view, scrolling if necessary.
	///
	/// Synchronizes the viewport visibility logic with the render pipeline by
	/// using the same gutter layout and text width calculation.
	pub fn reveal_cursor_in_view(&mut self, buffer_id: ViewId) {
		use xeno_registry::options::option_keys as opt_keys;
		let tab_width = self.resolve_typed_option(buffer_id, opt_keys::TAB_WIDTH) as usize;
		let scroll_margin = self.resolve_typed_option(buffer_id, opt_keys::SCROLL_MARGIN) as usize;
		let area = self.view_area(buffer_id);

		if let Some(buffer) = self.state.core.editor.buffers.get_buffer_mut(buffer_id) {
			let total_lines = buffer.with_doc(|doc| doc.content().len_lines());
			let is_diff_file = buffer.file_type().is_some_and(|ft| ft == "diff");
			let gutter = crate::window::GutterSelector::Registry;
			let effective_gutter = if is_diff_file {
				crate::render::BufferRenderContext::diff_gutter_selector(gutter)
			} else {
				gutter
			};

			let gutter_layout = crate::render::GutterLayout::from_selector(effective_gutter, total_lines, area.width);
			let text_width = area.width.saturating_sub(gutter_layout.total_width) as usize;

			crate::render::ensure_buffer_cursor_visible(buffer, area, text_width, tab_width, scroll_margin);
			self.state.runtime.effects.request_redraw();
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

#[cfg(all(test, feature = "lsp"))]
mod tests {
	use crate::Editor;
	use crate::overlay::controllers;

	/// Opens a rename overlay on a scratch editor and returns the input ViewId.
	fn open_rename_overlay(editor: &mut Editor, initial_word: &str) -> crate::buffer::ViewId {
		let buffer_id = editor.focused_view();
		let ctl = controllers::RenameOverlay::new(buffer_id, 0, initial_word.to_string());
		let mut interaction = editor.state.ui.overlay_system.take_interaction();
		assert!(interaction.open(editor, Box::new(ctl)));
		editor.state.ui.overlay_system.restore_interaction(interaction);

		let active = editor.state.ui.overlay_system.interaction().active().expect("overlay should be open");
		active.session.input
	}

	#[tokio::test]
	async fn prepare_rename_placeholder_replaces_untouched_prompt() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(80, 24);

		let input_id = open_rename_overlay(&mut editor, "myVar");

		// Prompt should contain "myVar" initially.
		let text = editor
			.state
			.core
			.editor
			.buffers
			.get_buffer(input_id)
			.unwrap()
			.with_doc(|doc| doc.content().to_string());
		assert_eq!(text.trim_end_matches('\n'), "myVar");

		// Apply prepare response with placeholder — should replace since prompt is untouched.
		let response = xeno_lsp::lsp_types::PrepareRenameResponse::RangeWithPlaceholder {
			range: xeno_lsp::lsp_types::Range::default(),
			placeholder: "serverName".into(),
		};
		editor.apply_prepare_rename_response(response, xeno_lsp::OffsetEncoding::Utf16, "myVar");

		let text = editor
			.state
			.core
			.editor
			.buffers
			.get_buffer(input_id)
			.unwrap()
			.with_doc(|doc| doc.content().to_string());
		assert_eq!(text.trim_end_matches('\n'), "serverName", "placeholder should replace untouched prompt");
	}

	#[tokio::test]
	async fn prepare_rename_placeholder_does_not_clobber_user_edits() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(80, 24);

		let input_id = open_rename_overlay(&mut editor, "myVar");

		// Simulate user editing the prompt to "userTyped".
		if let Some(buffer) = editor.state.core.editor.buffers.get_buffer_mut(input_id) {
			buffer.reset_content("userTyped");
		}

		// Apply prepare response with placeholder — should NOT replace since user edited.
		let response = xeno_lsp::lsp_types::PrepareRenameResponse::RangeWithPlaceholder {
			range: xeno_lsp::lsp_types::Range::default(),
			placeholder: "serverName".into(),
		};
		editor.apply_prepare_rename_response(response, xeno_lsp::OffsetEncoding::Utf16, "myVar");

		let text = editor
			.state
			.core
			.editor
			.buffers
			.get_buffer(input_id)
			.unwrap()
			.with_doc(|doc| doc.content().to_string());
		assert_eq!(text.trim_end_matches('\n'), "userTyped", "user edits must not be clobbered");
	}
}
