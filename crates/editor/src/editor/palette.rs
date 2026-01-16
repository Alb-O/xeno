//! Command palette integration with editor.

use xeno_primitives::Mode;
use xeno_registry::options::{OptionValue, keys};

use super::Editor;
use crate::palette::{Palette, PaletteState, palette_rect, palette_style};
use crate::window::{GutterSelector, Window};

impl Editor {
	/// Opens the command palette.
	///
	/// Creates a scratch buffer in a floating window for command input.
	/// Returns `false` if the palette is already open or window dimensions are unavailable.
	pub fn open_palette(&mut self) -> bool {
		if self
			.overlays
			.get::<PaletteState>()
			.is_some_and(|p| p.is_open())
		{
			return false;
		}

		let (width, height) = match (self.viewport.width, self.viewport.height) {
			(Some(w), Some(h)) => (w, h),
			_ => return false,
		};

		let rect = palette_rect(width, height);
		let buffer_id = self.buffers.create_scratch();
		self.buffers
			.get_buffer_mut(buffer_id)
			.expect("just created")
			.local_options
			.set(keys::CURSORLINE.untyped(), OptionValue::Bool(false));
		let window_id = self.create_floating_window(buffer_id, rect, palette_style());

		let Window::Floating(float) = self.windows.get_mut(window_id).expect("just created") else {
			unreachable!()
		};
		float.sticky = true;
		float.dismiss_on_blur = true;
		float.gutter = GutterSelector::Prompt('>');

		self.focus_floating_window(window_id);
		self.buffers
			.get_buffer_mut(buffer_id)
			.expect("just created")
			.input
			.set_mode(Mode::Insert);

		self.overlays.insert(PaletteState::Open(Palette {
			window_id,
			buffer_id,
		}));
		true
	}

	/// Closes the command palette without executing.
	pub fn close_palette(&mut self) {
		let Some(palette) = self.overlays.get::<PaletteState>().and_then(|p| p.active()) else {
			return;
		};
		let window_id = palette.window_id;
		let buffer_id = palette.buffer_id;

		self.close_floating_window(window_id);
		self.buffers.remove_buffer(buffer_id);
		self.overlays.insert(PaletteState::Closed);

		self.focus_base_window();
		self.focused_buffer_mut().input.set_mode(Mode::Normal);
	}

	/// Executes the command in the palette and closes it.
	///
	/// Parses the input as `<command> [args...]` and queues it for execution.
	/// Returns the raw input string on success, or `None` if the palette wasn't
	/// open, the input was empty, or the command was not found.
	pub fn execute_palette(&mut self) -> Option<String> {
		let buffer_id = self
			.overlays
			.get::<PaletteState>()
			.and_then(|p| p.buffer_id())?;
		let input = self
			.buffers
			.get_buffer(buffer_id)?
			.with_doc(|doc| doc.content().to_string());
		let input = input.trim().to_string();

		self.close_palette();

		if input.is_empty() {
			return None;
		}

		let mut parts = input.split_whitespace();
		let name = parts.next()?;
		let args: Vec<String> = parts.map(String::from).collect();

		// Check editor-direct commands first (e.g., LSP commands), then registry commands
		if let Some(cmd) = crate::commands::find_editor_command(name) {
			self.workspace.command_queue.push(cmd.name, args);
			Some(input)
		} else if let Some(cmd) = xeno_registry::commands::find_command(name) {
			self.workspace.command_queue.push(cmd.name, args);
			Some(input)
		} else {
			self.notify(xeno_registry::notifications::keys::unknown_command::call(
				name,
			));
			None
		}
	}

	/// Returns `true` if the palette is currently open.
	pub fn palette_is_open(&self) -> bool {
		self.overlays
			.get::<PaletteState>()
			.is_some_and(|p| p.is_open())
	}

	fn focus_floating_window(&mut self, window_id: crate::window::WindowId) {
		let Window::Floating(float) = self.windows.get(window_id).expect("window exists") else {
			return;
		};
		self.focus = crate::editor::FocusTarget::Buffer {
			window: window_id,
			buffer: float.buffer,
		};
		self.frame.needs_redraw = true;
	}

	fn focus_base_window(&mut self) {
		let base_id = self.windows.base_id();
		self.focus = crate::editor::FocusTarget::Buffer {
			window: base_id,
			buffer: self.base_window().focused_buffer,
		};
		self.frame.needs_redraw = true;
	}
}
