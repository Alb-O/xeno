//! Command palette for executing commands via floating input.
//!
//! The palette uses a scratch buffer as its input field, providing familiar
//! text editing controls. Commands are parsed and executed on Enter.

use xeno_primitives::Mode;
use xeno_registry::options::{OptionValue, keys};
use xeno_tui::layout::Rect;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::Editor;
use crate::buffer::BufferId;
use crate::window::{FloatingStyle, GutterSelector, Window, WindowId};

/// Active command palette instance.
#[derive(Debug)]
pub struct Palette {
	/// The floating window containing the input buffer.
	pub window_id: WindowId,
	/// The scratch buffer used for input.
	pub buffer_id: BufferId,
}

/// Palette lifecycle state.
#[derive(Debug, Default)]
pub enum PaletteState {
	/// No palette open.
	#[default]
	Closed,
	/// Palette is open and accepting input.
	Open(Palette),
}

impl PaletteState {
	/// Returns true if the palette is open.
	pub fn is_open(&self) -> bool {
		matches!(self, Self::Open(_))
	}

	/// Returns the active palette, if open.
	pub fn active(&self) -> Option<&Palette> {
		match self {
			Self::Open(p) => Some(p),
			Self::Closed => None,
		}
	}

	/// Returns the window ID if palette is open.
	pub fn window_id(&self) -> Option<WindowId> {
		self.active().map(|p| p.window_id)
	}

	/// Returns the buffer ID if palette is open.
	pub fn buffer_id(&self) -> Option<BufferId> {
		self.active().map(|p| p.buffer_id)
	}
}

/// Default floating style for the command palette.
pub fn palette_style() -> FloatingStyle {
	FloatingStyle {
		border: true,
		border_type: BorderType::Stripe,
		padding: Padding::horizontal(1),
		shadow: false,
		title: None,
	}
}

/// Computes the palette rectangle centered horizontally near the top.
pub fn palette_rect(screen_width: u16, screen_height: u16) -> Rect {
	let width = screen_width.saturating_sub(20).clamp(40, 80);
	let height = 3; // Border top + content + border bottom (padding is internal)
	let x = (screen_width.saturating_sub(width)) / 2;
	let y = screen_height / 5;

	Rect::new(x, y, width, height)
}

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
		let buffer_id = self.core.buffers.create_scratch();
		self.core
			.buffers
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
		self.core
			.buffers
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
		self.core.buffers.remove_buffer(buffer_id);
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
			.core
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
			self.core.workspace.command_queue.push(cmd.name, args);
			Some(input)
		} else if let Some(cmd) = xeno_registry::commands::find_command(name) {
			self.core.workspace.command_queue.push(cmd.name(), args);
			Some(input)
		} else {
			self.notify(xeno_registry::notifications::keys::unknown_command(
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

	fn focus_floating_window(&mut self, window_id: WindowId) {
		let Window::Floating(float) = self.windows.get(window_id).expect("window exists") else {
			return;
		};
		self.focus = crate::impls::FocusTarget::Buffer {
			window: window_id,
			buffer: float.buffer,
		};
		self.frame.needs_redraw = true;
	}

	fn focus_base_window(&mut self) {
		let base_id = self.windows.base_id();
		self.focus = crate::impls::FocusTarget::Buffer {
			window: base_id,
			buffer: self.base_window().focused_buffer,
		};
		self.frame.needs_redraw = true;
	}
}
