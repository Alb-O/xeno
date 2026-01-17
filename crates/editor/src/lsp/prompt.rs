//! Prompt overlay helpers.

use xeno_primitives::{Mode, Selection};
use xeno_registry_notifications::keys;

use crate::impls::Editor;
use crate::prompt::{Prompt, PromptKind, PromptState, prompt_rect, prompt_style};
use crate::window::{GutterSelector, Window};

impl Editor {
	pub fn open_rename_prompt(&mut self) -> bool {
		if self
			.overlays
			.get::<PromptState>()
			.is_some_and(|state| state.is_open())
		{
			return false;
		}

		let (width, height) = match (self.viewport.width, self.viewport.height) {
			(Some(w), Some(h)) => (w, h),
			_ => return false,
		};

		let buffer_id = self.focused_view();
		let (cursor, word, rename_supported) = {
			let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
				return false;
			};
			let rename_supported = self
				.lsp
				.prepare_position_request(buffer)
				.ok()
				.flatten()
				.is_some_and(|(client, _, _)| client.supports_rename());
			(buffer.cursor, word_at_cursor(buffer), rename_supported)
		};
		if !rename_supported {
			self.notify(keys::warn("Rename not supported for this buffer"));
			return false;
		}

		let rect = prompt_rect(width, height);
		let prompt_buffer_id = self.core.buffers.create_scratch();
		let window_id = self.create_floating_window(prompt_buffer_id, rect, prompt_style("Rename"));

		let Window::Floating(float) = self.windows.get_mut(window_id).expect("just created") else {
			unreachable!()
		};
		float.sticky = true;
		float.dismiss_on_blur = true;
		float.gutter = GutterSelector::Prompt('>');

		self.focus_prompt_window(window_id);
		if !word.is_empty() {
			let end = word.chars().count();
			let buffer = self
				.core
				.buffers
				.get_buffer_mut(prompt_buffer_id)
				.expect("prompt buffer exists");
			buffer.reset_content(word.as_str());
			buffer.set_cursor_and_selection(end, Selection::single(0, end));
		}
		self.core
			.buffers
			.get_buffer_mut(prompt_buffer_id)
			.expect("prompt buffer exists")
			.input
			.set_mode(Mode::Insert);

		self.overlays.insert(PromptState::Open(Prompt {
			window_id,
			buffer_id: prompt_buffer_id,
			kind: PromptKind::Rename {
				target_buffer: buffer_id,
				position: cursor,
			},
		}));
		true
	}

	pub fn prompt_is_open(&self) -> bool {
		self.overlays
			.get::<PromptState>()
			.is_some_and(|state| state.is_open())
	}

	pub fn close_prompt(&mut self) {
		let Some(prompt) = self
			.overlays
			.get::<PromptState>()
			.and_then(|state| state.active())
		else {
			return;
		};
		let window_id = prompt.window_id;
		let buffer_id = prompt.buffer_id;
		let target_buffer = match prompt.kind {
			PromptKind::Rename { target_buffer, .. } => target_buffer,
		};

		self.close_floating_window(window_id);
		self.core.buffers.remove_buffer(buffer_id);
		self.overlays.insert(PromptState::Closed);

		self.focus_view(target_buffer);
		if let Some(buffer) = self.core.buffers.get_buffer_mut(target_buffer) {
			buffer.input.set_mode(Mode::Normal);
		}
	}

	pub async fn execute_prompt(&mut self) {
		let Some(prompt) = self
			.overlays
			.get::<PromptState>()
			.and_then(|state| state.active())
			.cloned()
		else {
			return;
		};

		let input = self
			.core
			.buffers
			.get_buffer(prompt.buffer_id)
			.map(|buffer| buffer.with_doc(|doc| doc.content().to_string()))
			.unwrap_or_default();
		let input = input.trim().to_string();

		self.close_prompt();

		if input.is_empty() {
			return;
		}

		match prompt.kind {
			PromptKind::Rename {
				target_buffer,
				position,
			} => {
				self.apply_rename(target_buffer, position, input).await;
			}
		}
	}

	fn focus_prompt_window(&mut self, window_id: crate::window::WindowId) {
		let Window::Floating(float) = self.windows.get(window_id).expect("window exists") else {
			return;
		};
		self.focus = crate::impls::FocusTarget::Buffer {
			window: window_id,
			buffer: float.buffer,
		};
		self.frame.needs_redraw = true;
	}

	async fn apply_rename(
		&mut self,
		buffer_id: crate::buffer::BufferId,
		position: usize,
		new_name: String,
	) {
		let Some(buffer) = self.core.buffers.get_buffer(buffer_id) else {
			return;
		};
		if buffer.is_readonly() {
			self.notify(keys::BUFFER_READONLY);
			return;
		}
		let Some((client, uri, _)) = self.lsp.prepare_position_request(buffer).ok().flatten()
		else {
			self.notify(keys::warn("Rename not supported for this buffer"));
			return;
		};
		let encoding = client.offset_encoding();
		let Some(pos) = buffer
			.with_doc(|doc| xeno_lsp::char_to_lsp_position(doc.content(), position, encoding))
		else {
			self.notify(keys::error("Invalid rename position"));
			return;
		};

		match client.rename(uri, pos, new_name).await {
			Ok(Some(edit)) => {
				if let Err(err) = self.apply_workspace_edit(edit).await {
					self.notify(keys::error(err.to_string()));
				}
			}
			Ok(None) => {
				self.notify(keys::info("Rename not supported for this buffer"));
			}
			Err(err) => {
				self.notify(keys::error(err.to_string()));
			}
		}
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
