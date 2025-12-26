//! Generic panel wrapper for SplitBuffer implementations.
//!
//! This module provides `SplitBufferPanel<T>`, which wraps any type implementing
//! `SplitBuffer` and exposes it as a UI `Panel` for the dock system.

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Clear, Widget};
use termina::event::{KeyCode as TmKeyCode, KeyEvent, Modifiers as TmModifiers, MouseEvent};
use tome_manifest::{
	SplitAttrs, SplitBuffer, SplitColor, SplitCursorStyle, SplitDockPreference, SplitEventResult,
	SplitKey, SplitKeyCode, SplitModifiers, SplitMouse, SplitMouseAction, SplitMouseButton,
	SplitSize,
};
use tome_theme::Theme;

use super::FocusTarget;
use super::dock::DockSlot;
use super::keymap::UiKeyChord;
use super::panel::{CursorRequest, EventResult, Panel, PanelInitContext, UiEvent, UiRequest};
use crate::editor::Editor;

/// Configuration for a SplitBufferPanel.
pub struct SplitBufferPanelConfig {
	/// Panel ID (must be unique).
	pub id: String,
	/// Optional global keybinding to toggle this panel.
	pub toggle_keybinding: Option<UiKeyChord>,
	/// Priority for the toggle keybinding (higher = more specific).
	pub keybinding_priority: i16,
}

impl SplitBufferPanelConfig {
	pub fn new(id: impl Into<String>) -> Self {
		Self {
			id: id.into(),
			toggle_keybinding: None,
			keybinding_priority: 100,
		}
	}

	pub fn with_toggle(mut self, keybinding: UiKeyChord) -> Self {
		self.toggle_keybinding = Some(keybinding);
		self
	}

	pub fn with_priority(mut self, priority: i16) -> Self {
		self.keybinding_priority = priority;
		self
	}
}

/// A panel that wraps a `SplitBuffer` implementation.
///
/// This provides the bridge between the abstract `SplitBuffer` trait and
/// the concrete `Panel` trait used by the UI dock system.
pub struct SplitBufferPanel<T: SplitBuffer> {
	config: SplitBufferPanelConfig,
	buffer: T,
	last_tick: Instant,
	current_area: Rect,
}

impl<T: SplitBuffer> SplitBufferPanel<T> {
	/// Creates a new panel wrapping the given buffer.
	pub fn new(config: SplitBufferPanelConfig, buffer: T) -> Self {
		Self {
			config,
			buffer,
			last_tick: Instant::now(),
			current_area: Rect::default(),
		}
	}

	/// Returns a reference to the wrapped buffer.
	pub fn buffer(&self) -> &T {
		&self.buffer
	}

	/// Returns a mutable reference to the wrapped buffer.
	pub fn buffer_mut(&mut self) -> &mut T {
		&mut self.buffer
	}

	fn convert_key(key: &KeyEvent) -> Option<SplitKey> {
		let code = match key.code {
			TmKeyCode::Char(c) => SplitKeyCode::Char(c),
			TmKeyCode::Enter => SplitKeyCode::Enter,
			TmKeyCode::Escape => SplitKeyCode::Escape,
			TmKeyCode::Backspace => SplitKeyCode::Backspace,
			TmKeyCode::Tab => SplitKeyCode::Tab,
			TmKeyCode::Up => SplitKeyCode::Up,
			TmKeyCode::Down => SplitKeyCode::Down,
			TmKeyCode::Left => SplitKeyCode::Left,
			TmKeyCode::Right => SplitKeyCode::Right,
			TmKeyCode::Home => SplitKeyCode::Home,
			TmKeyCode::End => SplitKeyCode::End,
			TmKeyCode::PageUp => SplitKeyCode::PageUp,
			TmKeyCode::PageDown => SplitKeyCode::PageDown,
			TmKeyCode::Delete => SplitKeyCode::Delete,
			TmKeyCode::Insert => SplitKeyCode::Insert,
			_ => return None,
		};

		let mut modifiers = SplitModifiers::NONE;
		if key.modifiers.contains(TmModifiers::CONTROL) {
			modifiers = modifiers.union(SplitModifiers::CTRL);
		}
		if key.modifiers.contains(TmModifiers::ALT) {
			modifiers = modifiers.union(SplitModifiers::ALT);
		}
		if key.modifiers.contains(TmModifiers::SHIFT) {
			modifiers = modifiers.union(SplitModifiers::SHIFT);
		}

		Some(SplitKey::new(code, modifiers))
	}

	#[allow(dead_code, reason = "mouse support will be added when needed")]
	fn convert_mouse(mouse: &MouseEvent, area: Rect) -> Option<SplitMouse> {
		use termina::event::{MouseButton, MouseEventKind};

		// Convert to buffer-relative coordinates
		if mouse.column < area.x || mouse.row < area.y {
			return None;
		}
		let col = mouse.column.saturating_sub(area.x);
		let row = mouse.row.saturating_sub(area.y);
		if col >= area.width || row >= area.height {
			return None;
		}

		let button = |btn: MouseButton| match btn {
			MouseButton::Left => SplitMouseButton::Left,
			MouseButton::Right => SplitMouseButton::Right,
			MouseButton::Middle => SplitMouseButton::Middle,
		};

		let action = match mouse.kind {
			MouseEventKind::Down(btn) => SplitMouseAction::Press(button(btn)),
			MouseEventKind::Up(btn) => SplitMouseAction::Release(button(btn)),
			MouseEventKind::Drag(btn) => SplitMouseAction::Drag(button(btn)),
			MouseEventKind::ScrollUp => SplitMouseAction::ScrollUp,
			MouseEventKind::ScrollDown => SplitMouseAction::ScrollDown,
			_ => return None,
		};

		Some(SplitMouse {
			position: tome_base::Position::new(col, row),
			action,
		})
	}

	fn apply_result(&self, result: SplitEventResult) -> EventResult {
		let mut event_result = if result.consumed {
			EventResult::consumed()
		} else {
			EventResult::not_consumed()
		};

		if result.needs_redraw {
			event_result = event_result.with_request(UiRequest::Redraw);
		}

		if result.release_focus {
			event_result = event_result.with_request(UiRequest::Focus(FocusTarget::editor()));
		}

		if result.wants_close {
			event_result = event_result.with_request(UiRequest::ClosePanel(self.config.id.clone()));
		}

		event_result
	}
}

impl<T: SplitBuffer + 'static> Panel for SplitBufferPanel<T> {
	fn id(&self) -> &str {
		&self.config.id
	}

	fn default_slot(&self) -> DockSlot {
		match T::dock_preference() {
			SplitDockPreference::Bottom => DockSlot::Bottom,
			SplitDockPreference::Top => DockSlot::Top,
			SplitDockPreference::Left => DockSlot::Left,
			SplitDockPreference::Right => DockSlot::Right,
		}
	}

	fn on_register(&mut self, ctx: PanelInitContext<'_>) {
		if let Some(keybinding) = &self.config.toggle_keybinding {
			ctx.keybindings.register_global(
				*keybinding,
				self.config.keybinding_priority,
				vec![UiRequest::TogglePanel(self.config.id.clone())],
			);
		}
	}

	fn on_open_changed(&mut self, open: bool) {
		if open {
			self.buffer.on_open();
		} else {
			self.buffer.on_close();
		}
	}

	fn on_focus_changed(&mut self, focused: bool) {
		self.buffer.on_focus_changed(focused);
	}

	fn cursor_style_when_focused(&self) -> Option<termina::style::CursorStyle> {
		self.buffer.cursor().map(|c| match c.style {
			SplitCursorStyle::Hidden | SplitCursorStyle::Default => {
				termina::style::CursorStyle::Default
			}
			SplitCursorStyle::Block => termina::style::CursorStyle::SteadyBlock,
			SplitCursorStyle::Bar => termina::style::CursorStyle::BlinkingBar,
			SplitCursorStyle::Underline => termina::style::CursorStyle::SteadyUnderline,
		})
	}

	fn handle_event(
		&mut self,
		event: UiEvent,
		_editor: &mut Editor,
		_focused: bool,
	) -> EventResult {
		match event {
			UiEvent::Tick => {
				let now = Instant::now();
				let delta = now.duration_since(self.last_tick);
				self.last_tick = now;

				let result = self.buffer.tick(delta);
				self.apply_result(result)
			}
			UiEvent::Key(key) => {
				// Escape always releases focus
				if matches!(key.code, TmKeyCode::Escape) {
					return EventResult::consumed()
						.with_request(UiRequest::Focus(FocusTarget::editor()));
				}

				if let Some(split_key) = Self::convert_key(&key) {
					let result = self.buffer.handle_key(split_key);
					self.apply_result(result)
				} else {
					EventResult::not_consumed()
				}
			}
			UiEvent::Paste(content) => {
				let result = self.buffer.handle_paste(&content);
				self.apply_result(result)
			}
			UiEvent::Mouse(_mouse) => {
				// Mouse support can be added later
				EventResult::consumed()
			}
			UiEvent::Resize => {
				// Resize is handled in render when we know the actual area
				EventResult::not_consumed()
			}
		}
	}

	fn render(
		&mut self,
		frame: &mut ratatui::Frame<'_>,
		area: Rect,
		_editor: &mut Editor,
		focused: bool,
		theme: &Theme,
	) -> Option<CursorRequest> {
		if area.width == 0 || area.height == 0 {
			return None;
		}

		let new_size = SplitSize::new(area.width, area.height);
		if self.current_area != area {
			self.current_area = area;
			self.buffer.resize(new_size);
		}

		frame.render_widget(Clear, area);
		let base_style =
			Style::default()
				.bg(theme.colors.popup.bg.into())
				.fg(theme.colors.popup.fg.into());
		frame.render_widget(Block::default().style(base_style), area);

		let widget = SplitBufferWidget {
			buffer: &self.buffer,
			base_style,
		};
		frame.render_widget(widget, area);
		if focused
			&& let Some(cursor) = self.buffer.cursor()
			&& !matches!(cursor.style, SplitCursorStyle::Hidden)
			&& cursor.row < area.height
			&& cursor.col < area.width
		{
			return Some(CursorRequest {
				pos: Position {
					x: area.x + cursor.col,
					y: area.y + cursor.row,
				},
				style: Some(match cursor.style {
					SplitCursorStyle::Hidden | SplitCursorStyle::Default => {
						termina::style::CursorStyle::Default
					}
					SplitCursorStyle::Block => termina::style::CursorStyle::SteadyBlock,
					SplitCursorStyle::Bar => termina::style::CursorStyle::BlinkingBar,
					SplitCursorStyle::Underline => termina::style::CursorStyle::SteadyUnderline,
				}),
			});
		}

		None
	}
}

/// Widget for rendering a SplitBuffer's cells.
struct SplitBufferWidget<'a, T: SplitBuffer> {
	buffer: &'a T,
	base_style: Style,
}

impl<T: SplitBuffer> Widget for SplitBufferWidget<'_, T> {
	fn render(self, area: Rect, buf: &mut Buffer) {
		for y in area.top()..area.bottom() {
			for x in area.left()..area.right() {
				buf[(x, y)].set_style(self.base_style);
			}
		}

		self.buffer.for_each_cell(|row, col, cell| {
			if row >= area.height || col >= area.width {
				return;
			}
			if cell.wide_continuation {
				return;
			}

			let x = area.x + col;
			let y = area.y + row;

			let mut style = self.base_style;

			if let Some(fg) = cell.fg {
				style = style.fg(convert_color(fg));
			}
			if let Some(bg) = cell.bg {
				style = style.bg(convert_color(bg));
			}

			let mut mods = Modifier::empty();
			if cell.attrs.contains(SplitAttrs::BOLD) {
				mods |= Modifier::BOLD;
			}
			if cell.attrs.contains(SplitAttrs::ITALIC) {
				mods |= Modifier::ITALIC;
			}
			if cell.attrs.contains(SplitAttrs::UNDERLINE) {
				mods |= Modifier::UNDERLINED;
			}
			style = style.add_modifier(mods);

			if cell.attrs.contains(SplitAttrs::INVERSE) {
				let fg = style.fg;
				let bg = style.bg;
				style = style.fg(bg.unwrap_or(Color::Reset));
				style = style.bg(fg.unwrap_or(Color::Reset));
			}

			let out = &mut buf[(x, y)];
			out.set_style(style);
			if cell.symbol.is_empty() {
				out.set_symbol(" ");
			} else {
				out.set_symbol(&cell.symbol);
			}
		});
	}
}

fn convert_color(color: SplitColor) -> Color {
	match color {
		SplitColor::Indexed(i) => Color::Indexed(i),
		SplitColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
	}
}

/// Extension trait for SplitBuffer to provide dock preference.
///
/// This is a separate trait because it needs to be a static method,
/// which can't be in the main trait without making it non-object-safe.
pub trait SplitBufferExt {
	fn dock_preference() -> SplitDockPreference {
		SplitDockPreference::Bottom
	}
}

// Implement for all SplitBuffer types with a default
impl<T: SplitBuffer> SplitBufferExt for T {}
