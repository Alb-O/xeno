//! Prompt overlay helpers for search and other interactive inputs.

use regex::Regex;
use xeno_primitives::range::Range;
use xeno_primitives::{Mode, Selection};
use xeno_registry::notifications::keys;
use xeno_registry::options::keys as opt_keys;

use crate::buffer::ViewId;
use crate::impls::Editor;
use crate::movement;
use crate::prompt::{Prompt, PromptKind, PromptState, SearchPromptRuntime, prompt_rect, prompt_style};
use crate::render::ensure_buffer_cursor_visible;
use crate::window::{GutterSelector, Window};

const PREVIEW_WINDOW_CHARS: usize = 200_000;
const FULL_SCAN_PREVIEW_MAX: usize = 500_000;

impl Editor {
	pub fn open_rename_prompt(&mut self) -> bool {
		if self
			.state
			.overlays
			.get::<PromptState>()
			.is_some_and(|state| state.is_open())
		{
			return false;
		}

		let (width, height) = match (self.state.viewport.width, self.state.viewport.height) {
			(Some(w), Some(h)) => (w, h),
			_ => return false,
		};

		let buffer_id = self.focused_view();
		let (cursor, word, rename_supported) = {
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

			(buffer.cursor, word_at_cursor(buffer), rename_supported)
		};

		if !rename_supported {
			self.notify(keys::warn("Rename not supported for this buffer"));
			return false;
		}

		let rect = prompt_rect(width, height);
		let prompt_buffer_id = self.state.core.buffers.create_scratch();
		let window_id = self.create_floating_window(prompt_buffer_id, rect, prompt_style("Rename"));

		let Window::Floating(float) = self.state.windows.get_mut(window_id).expect("just created")
		else {
			unreachable!()
		};
		float.sticky = true;
		float.dismiss_on_blur = true;
		float.gutter = GutterSelector::Prompt('>');

		self.focus_prompt_window(window_id);
		if !word.is_empty() {
			let end = word.chars().count();
			let buffer = self
				.state
				.core
				.buffers
				.get_buffer_mut(prompt_buffer_id)
				.expect("prompt buffer exists");
			buffer.reset_content(word.as_str());
			buffer.set_cursor_and_selection(end, Selection::single(0, end));
		}
		self.state
			.core
			.buffers
			.get_buffer_mut(prompt_buffer_id)
			.expect("prompt buffer exists")
			.input
			.set_mode(Mode::Insert);

		self.state.overlays.insert(PromptState::Open {
			prompt: Prompt {
				window_id,
				buffer_id: prompt_buffer_id,
				kind: PromptKind::Rename {
					target_buffer: buffer_id,
					position: cursor,
				},
			},
			search: None,
		});
		true
	}

	/// Opens a search prompt for finding text in the current buffer.
	///
	/// Incremental preview is enabled while typing. Pressing `Enter` confirms the
	/// search and saves the pattern.
	///
	/// # Arguments
	///
	/// * `reverse` - If true, performs a backward search from the cursor.
	pub fn open_search_prompt(&mut self, reverse: bool) -> bool {
		if self
			.state
			.overlays
			.get::<PromptState>()
			.is_some_and(|state| state.is_open())
		{
			return false;
		}

		let (width, height) = match (self.state.viewport.width, self.state.viewport.height) {
			(Some(w), Some(h)) => (w, h),
			_ => return false,
		};

		let target_buffer = self.focused_view();
		let (origin_cursor, origin_selection) = {
			let Some(buffer) = self.state.core.buffers.get_buffer(target_buffer) else {
				return false;
			};
			(buffer.cursor, buffer.selection.clone())
		};

		let rect = prompt_rect(width, height);
		let prompt_buffer_id = self.state.core.buffers.create_scratch();
		let title = if reverse { "Search (reverse)" } else { "Search" };
		let window_id = self.create_floating_window(prompt_buffer_id, rect, prompt_style(title));

		let Window::Floating(float) = self.state.windows.get_mut(window_id).expect("just created")
		else {
			unreachable!()
		};
		float.sticky = true;
		float.dismiss_on_blur = true;
		float.gutter = GutterSelector::Prompt(if reverse { '?' } else { '/' });

		self.focus_prompt_window(window_id);

		self.state
			.core
			.buffers
			.get_buffer_mut(prompt_buffer_id)
			.expect("prompt buffer exists")
			.input
			.set_mode(Mode::Insert);

		self.state.overlays.insert(PromptState::Open {
			prompt: Prompt {
				window_id,
				buffer_id: prompt_buffer_id,
				kind: PromptKind::Search {
					target_buffer,
					reverse,
				},
			},
			search: Some(SearchPromptRuntime {
				origin_cursor,
				origin_selection,
				last_input: String::new(),
				last_preview: None,
				cached: None,
				last_error: None,
			}),
		});
		true
	}

	pub fn prompt_is_open(&self) -> bool {
		self.state
			.overlays
			.get::<PromptState>()
			.is_some_and(|state| state.is_open())
	}

	pub fn close_prompt(&mut self, restore_origin: bool) {
		let Some((prompt, search_rt)) = self
			.state
			.overlays
			.get_mut::<PromptState>()
			.and_then(|state| state.take_open())
		else {
			return;
		};
		let window_id = prompt.window_id;
		let buffer_id = prompt.buffer_id;

		let (target_buffer, restore_sel) = match (&prompt.kind, search_rt.as_ref()) {
			(PromptKind::Rename { target_buffer, .. }, _) => (target_buffer.clone(), None),
			(PromptKind::Search { target_buffer, .. }, Some(rt)) => (
				target_buffer.clone(),
				if restore_origin {
					Some((rt.origin_cursor, rt.origin_selection.clone()))
				} else {
					None
				},
			),
			(PromptKind::Search { target_buffer, .. }, None) => (target_buffer.clone(), None),
		};

		self.close_floating_window(window_id);
		self.state.core.buffers.remove_buffer(buffer_id);

		self.focus_view(target_buffer);
		if let Some(buffer) = self.state.core.buffers.get_buffer_mut(target_buffer) {
			buffer.input.set_mode(Mode::Normal);
			if let Some((cursor, selection)) = restore_sel {
				buffer.set_cursor(cursor);
				buffer.set_selection(selection);
			}
		}
		self.state.frame.needs_redraw = true;
	}

	pub async fn execute_prompt(&mut self) {
		let Some((prompt, search_rt)) = self
			.state
			.overlays
			.get_mut::<PromptState>()
			.and_then(|state| state.take_open())
		else {
			return;
		};

		let input = self
			.state
			.core
			.buffers
			.get_buffer(prompt.buffer_id)
			.map(|buffer| buffer.with_doc(|doc| doc.content().to_string()))
			.unwrap_or_default();
		let input = input.trim_end_matches('\n').trim().to_string();

		match prompt.kind {
			PromptKind::Rename {
				target_buffer,
				position,
			} => {
				self.close_prompt(false);
				if input.is_empty() {
					return;
				}
				self.apply_rename(target_buffer, position, input).await;
			}
			PromptKind::Search {
				target_buffer,
				reverse,
			} => {
				if input.is_empty() {
					self.close_prompt(true);
					return;
				}

				let origin_cursor = search_rt.as_ref().map(|rt| rt.origin_cursor).unwrap_or(0);

				let (result, err) = self.buffer_for(target_buffer).with_doc(|doc| {
					let text = doc.content().slice(..);
					if reverse {
						movement::find_prev(text, &input, origin_cursor)
					} else {
						movement::find_next(text, &input, origin_cursor + 1)
					}
				})
				.map(|opt| (opt, None))
				.unwrap_or_else(|e| (None, Some(e.to_string())));

				match (result, err) {
					(_, Some(e)) => {
						self.notify(keys::regex_error(&e));
						self.close_prompt(true);
					}
					(Some(range), None) => {
						if let Some(buffer) = self.state.core.buffers.get_buffer_mut(target_buffer) {
							buffer.input.set_last_search(input.clone(), reverse);
							let start = range.min();
							let end = range.max();
							buffer.set_cursor(start);
							buffer.set_selection(Selection::single(start, end));
						}
						self.reveal_cursor_in_view(target_buffer);
						self.close_prompt(false);
					}
					(None, None) => {
						if let Some(buffer) = self.state.core.buffers.get_buffer_mut(target_buffer) {
							buffer.input.set_last_search(input.clone(), reverse);
						}
						self.notify(keys::PATTERN_NOT_FOUND);
						self.close_prompt(true);
					}
				}
			}
		}
	}

	pub fn update_overlays_after_input(&mut self) {
		self.update_search_prompt_preview();
	}

	fn update_search_prompt_preview(&mut self) {
		let mut needs_redraw = false;
		let mut notify_msg = None;

		{
			let Some((prompt, Some(rt))) = self
				.state
				.overlays
				.get_mut::<PromptState>()
				.and_then(|state| state.active_mut())
			else {
				return;
			};

			let PromptKind::Search {
				target_buffer,
				reverse: _,
			} = prompt.kind
			else {
				return;
			};

			let input = self
				.state
				.core
				.buffers
				.get_buffer(prompt.buffer_id)
				.map(|buffer| buffer.with_doc(|doc| doc.content().to_string()))
				.unwrap_or_default();
			let input = input.trim_end_matches('\n').to_string();

			if input == rt.last_input {
				return;
			}
			rt.last_input = input.clone();

			if input.trim().is_empty() {
				let origin_cursor = rt.origin_cursor;
				let origin_selection = rt.origin_selection.clone();
				if let Some(buffer) = self.state.core.buffers.get_buffer_mut(target_buffer) {
					buffer.set_cursor(origin_cursor);
					buffer.set_selection(origin_selection);
				}
				rt.last_preview = None;
				rt.last_error = None;
				rt.cached = None;
				self.state.frame.needs_redraw = true;
				return;
			}

			let is_cached = rt.cached.as_ref().map_or(false, |(p, _)| p == &input);
			if !is_cached {
				match Regex::new(&input) {
					Ok(re) => {
						rt.cached = Some((input.clone(), re));
					}
					Err(e) => {
						let msg = e.to_string();
						if rt.last_error.as_deref() != Some(msg.as_str()) {
							rt.last_error = Some(msg.clone());
							notify_msg = Some(msg);
						}
						let origin_cursor = rt.origin_cursor;
						let origin_selection = rt.origin_selection.clone();
						if let Some(buffer) = self.state.core.buffers.get_buffer_mut(target_buffer) {
							buffer.set_cursor(origin_cursor);
							buffer.set_selection(origin_selection);
						}
						rt.last_preview = None;
						self.state.frame.needs_redraw = true;
					}
				}
			}
		};

		if let Some(msg) = notify_msg {
			self.notify(keys::regex_error(&msg));
			return;
		}

		let (target_buffer, reverse, origin_cursor, origin_selection, re) = {
			let Some((prompt, rt)) = self
				.state
				.overlays
				.get::<PromptState>()
				.and_then(|s| match s {
					PromptState::Open {
						prompt,
						search: Some(rt),
					} => Some((prompt, rt)),
					_ => None,
				})
			else {
				return;
			};
			let PromptKind::Search {
				target_buffer,
				reverse,
			} = prompt.kind
			else {
				return;
			};
			let Some((_, re)) = &rt.cached else {
				return;
			};
			(
				target_buffer,
				reverse,
				rt.origin_cursor,
				rt.origin_selection.clone(),
				re.clone(),
			)
		};

		let found = self.search_preview_find(target_buffer, &re, reverse, origin_cursor);

		let mut reveal = false;
		let mut notify_msg = None;

		if let Some((_, Some(rt))) = self
			.state
			.overlays
			.get_mut::<PromptState>()
			.and_then(|state| state.active_mut())
		{
			match found {
				Ok(Some(range)) => {
					if rt.last_preview != Some(range) {
						if let Some(buffer) = self.state.core.buffers.get_buffer_mut(target_buffer) {
							let start = range.min();
							let end = range.max();
							buffer.set_cursor(start);
							buffer.set_selection(Selection::single(start, end));
						}
						rt.last_preview = Some(range);
						reveal = true;
						needs_redraw = true;
					}
				}
				Ok(None) => {
					if rt.last_preview.is_some() {
						if let Some(buffer) = self.state.core.buffers.get_buffer_mut(target_buffer) {
							buffer.set_cursor(origin_cursor);
							buffer.set_selection(origin_selection);
						}
						rt.last_preview = None;
						needs_redraw = true;
					}
				}
				Err(e) => {
					let msg = e.to_string();
					if rt.last_error.as_deref() != Some(msg.as_str()) {
						rt.last_error = Some(msg.clone());
						notify_msg = Some(msg);
					}
				}
			}
		}

		if let Some(msg) = notify_msg {
			self.notify(keys::regex_error(&msg));
		}
		if reveal {
			self.reveal_cursor_in_view(target_buffer);
		}

		if needs_redraw {
			self.state.frame.needs_redraw = true;
		}
	}

	fn search_preview_find(
		&self,
		target_buffer: ViewId,
		re: &Regex,
		reverse: bool,
		origin_cursor: usize,
	) -> Result<Option<Range>, regex::Error> {
		let Some(buffer) = self.state.core.buffers.get_buffer(target_buffer) else {
			return Ok(None);
		};

		buffer.with_doc(|doc| {
			let content = doc.content();
			let len = content.len_chars();

			if len <= FULL_SCAN_PREVIEW_MAX {
				let slice = content.slice(..);
				return if reverse {
					Ok(movement::find_prev_re(slice, re, origin_cursor))
				} else {
					Ok(movement::find_next_re(slice, re, origin_cursor + 1))
				};
			}

			if reverse {
				let end = origin_cursor.min(len);
				let start = end.saturating_sub(PREVIEW_WINDOW_CHARS);
				let slice = content.slice(start..end);
				let rel_cursor = end - start;
				Ok(movement::find_prev_re(slice, re, rel_cursor).map(|r| offset_range(r, start)))
			} else {
				let start = (origin_cursor + 1).min(len);
				let end = (start + PREVIEW_WINDOW_CHARS).min(len);
				let slice = content.slice(start..end);
				Ok(movement::find_next_re(slice, re, 0).map(|r| offset_range(r, start)))
			}
		})
	}

	pub fn reveal_cursor_in_view(&mut self, buffer_id: ViewId) {
		let tab_width = self.resolve_typed_option(buffer_id, opt_keys::TAB_WIDTH) as usize;
		let scroll_margin = self.resolve_typed_option(buffer_id, opt_keys::SCROLL_MARGIN) as usize;
		let area = self.view_area(buffer_id);

		if let Some(buffer) = self.state.core.buffers.get_buffer_mut(buffer_id) {
			ensure_buffer_cursor_visible(buffer, area, tab_width, scroll_margin);
			self.state.frame.needs_redraw = true;
		}
	}

	fn buffer_for(&self, view_id: ViewId) -> &crate::buffer::Buffer {
		self.state.core.buffers.get_buffer(view_id).expect("buffer exists")
	}

	fn focus_prompt_window(&mut self, window_id: crate::window::WindowId) {
		let Window::Floating(float) = self.state.windows.get(window_id).expect("window exists")
		else {
			return;
		};
		self.state.focus = crate::impls::FocusTarget::Buffer {
			window: window_id,
			buffer: float.buffer,
		};
		self.state.frame.needs_redraw = true;
	}

	async fn apply_rename(
		&mut self,
		buffer_id: ViewId,
		position: usize,
		new_name: String,
	) {
		#[cfg(feature = "lsp")]
		{
			let Some(buffer) = self.state.core.buffers.get_buffer(buffer_id) else {
				return;
			};
			if buffer.is_readonly() {
				self.notify(keys::BUFFER_READONLY);
				return;
			}
			let Some((client, uri, _)) = self
				.state
				.lsp
				.prepare_position_request(buffer)
				.ok()
				.flatten()
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
		#[cfg(not(feature = "lsp"))]
		{
			let _ = (buffer_id, position, new_name);
			self.notify(keys::warn("LSP not enabled"));
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

fn offset_range(mut r: Range, base: usize) -> Range {
	r.anchor += base;
	r.head += base;
	r
}
