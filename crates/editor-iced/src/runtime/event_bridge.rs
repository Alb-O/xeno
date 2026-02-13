use iced::{Event, keyboard, mouse, window};
use iced_core::input_method;
use xeno_editor::runtime::RuntimeEvent;
use xeno_primitives::{Key, KeyCode, Modifiers, MouseButton as CoreMouseButton, MouseEvent as CoreMouseEvent, ScrollDirection};

const DEFAULT_TEXT_SIZE_PX: f32 = 16.0;
const DEFAULT_LINE_HEIGHT_FACTOR: f32 = 1.3;
const DEFAULT_MONOSPACE_WIDTH_FACTOR: f32 = 0.6;

const DEFAULT_CELL_WIDTH_PX: f32 = DEFAULT_TEXT_SIZE_PX * DEFAULT_MONOSPACE_WIDTH_FACTOR;
const DEFAULT_CELL_HEIGHT_PX: f32 = DEFAULT_TEXT_SIZE_PX * DEFAULT_LINE_HEIGHT_FACTOR;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CellMetrics {
	width_px: f32,
	height_px: f32,
}

impl CellMetrics {
	pub(crate) fn from_env() -> Self {
		Self {
			width_px: parse_cell_size(std::env::var("XENO_ICED_CELL_WIDTH_PX").ok(), DEFAULT_CELL_WIDTH_PX),
			height_px: parse_cell_size(std::env::var("XENO_ICED_CELL_HEIGHT_PX").ok(), DEFAULT_CELL_HEIGHT_PX),
		}
	}

	pub(crate) fn to_grid(self, logical_width_px: f32, logical_height_px: f32) -> (u16, u16) {
		(
			logical_pixels_to_cells(logical_width_px, self.width_px),
			logical_pixels_to_cells(logical_height_px, self.height_px),
		)
	}

	pub(crate) fn height_px(self) -> f32 {
		self.height_px
	}
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EventBridgeState {
	mouse_row: u16,
	mouse_col: u16,
	mouse_button: Option<CoreMouseButton>,
	modifiers: Modifiers,
	ime_preedit: Option<String>,
}

impl EventBridgeState {
	pub(crate) fn ime_preedit(&self) -> Option<&str> {
		self.ime_preedit.as_deref()
	}
}

pub(crate) fn map_event(event: Event, cell_metrics: CellMetrics, event_state: &mut EventBridgeState) -> Option<RuntimeEvent> {
	match event {
		Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
			event_state.modifiers = map_modifiers(modifiers);
			None
		}
		Event::Keyboard(keyboard::Event::KeyPressed {
			modified_key,
			physical_key,
			modifiers,
			..
		}) => {
			event_state.modifiers = map_modifiers(modifiers);
			map_key_event(modified_key, physical_key, modifiers).map(RuntimeEvent::Key)
		}
		Event::Keyboard(keyboard::Event::KeyReleased { modifiers, .. }) => {
			event_state.modifiers = map_modifiers(modifiers);
			None
		}
		Event::Mouse(mouse::Event::CursorMoved { position }) => {
			let col = logical_pixels_to_cell_index(position.x, cell_metrics.width_px);
			let row = logical_pixels_to_cell_index(position.y, cell_metrics.height_px);
			event_state.mouse_col = col;
			event_state.mouse_row = row;

			Some(RuntimeEvent::Mouse(match event_state.mouse_button {
				Some(button) => CoreMouseEvent::Drag {
					button,
					row,
					col,
					modifiers: event_state.modifiers,
				},
				None => CoreMouseEvent::Move { row, col },
			}))
		}
		Event::Mouse(mouse::Event::ButtonPressed(button)) => {
			let button = map_mouse_button(button)?;
			event_state.mouse_button = Some(button);

			Some(RuntimeEvent::Mouse(CoreMouseEvent::Press {
				button,
				row: event_state.mouse_row,
				col: event_state.mouse_col,
				modifiers: event_state.modifiers,
			}))
		}
		Event::Mouse(mouse::Event::ButtonReleased(_)) => {
			event_state.mouse_button = None;
			Some(RuntimeEvent::Mouse(CoreMouseEvent::Release {
				row: event_state.mouse_row,
				col: event_state.mouse_col,
			}))
		}
		Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
			let direction = map_scroll_delta(delta)?;
			Some(RuntimeEvent::Mouse(CoreMouseEvent::Scroll {
				direction,
				row: event_state.mouse_row,
				col: event_state.mouse_col,
				modifiers: event_state.modifiers,
			}))
		}
		Event::Window(window::Event::Opened { size, .. }) | Event::Window(window::Event::Resized(size)) => {
			let (cols, rows) = cell_metrics.to_grid(size.width, size.height);
			Some(RuntimeEvent::WindowResized { cols, rows })
		}
		Event::InputMethod(event) => map_input_method_event(event, event_state),
		Event::Window(window::Event::Focused) => Some(RuntimeEvent::FocusIn),
		Event::Window(window::Event::Unfocused) => Some(RuntimeEvent::FocusOut),
		_ => None,
	}
}

fn map_input_method_event(event: input_method::Event, event_state: &mut EventBridgeState) -> Option<RuntimeEvent> {
	match event {
		input_method::Event::Opened | input_method::Event::Closed => {
			event_state.ime_preedit = None;
			None
		}
		input_method::Event::Preedit(text, _selection) => {
			event_state.ime_preedit = if text.is_empty() { None } else { Some(text) };
			None
		}
		input_method::Event::Commit(text) if !text.is_empty() => {
			event_state.ime_preedit = None;
			Some(RuntimeEvent::Paste(text))
		}
		input_method::Event::Commit(_) => {
			event_state.ime_preedit = None;
			None
		}
	}
}

fn logical_pixels_to_cells(logical_px: f32, cell_px: f32) -> u16 {
	if !logical_px.is_finite() || !cell_px.is_finite() || cell_px <= 0.0 {
		return 1;
	}

	let cells = (logical_px / cell_px).floor();
	cells.clamp(1.0, u16::MAX as f32) as u16
}

fn logical_pixels_to_cell_index(logical_px: f32, cell_px: f32) -> u16 {
	if !logical_px.is_finite() || !cell_px.is_finite() || cell_px <= 0.0 {
		return 0;
	}

	let cell_index = (logical_px / cell_px).floor();
	cell_index.clamp(0.0, u16::MAX as f32) as u16
}

fn map_modifiers(modifiers: keyboard::Modifiers) -> Modifiers {
	Modifiers {
		ctrl: modifiers.control(),
		alt: modifiers.alt(),
		shift: modifiers.shift(),
	}
}

fn parse_cell_size(value: Option<String>, default: f32) -> f32 {
	let Some(value) = value else {
		return default;
	};

	match value.parse::<f32>() {
		Ok(px) if px.is_finite() && px > 0.0 => px,
		_ => default,
	}
}

fn map_key_event(key: keyboard::Key, physical_key: keyboard::key::Physical, modifiers: keyboard::Modifiers) -> Option<Key> {
	let modifiers = map_modifiers(modifiers);

	let code = match key.as_ref() {
		keyboard::Key::Character(chars) => {
			let mut it = chars.chars();
			let ch = it.next().or_else(|| key.to_latin(physical_key))?;
			if it.next().is_some() {
				return None;
			}
			KeyCode::Char(ch)
		}
		keyboard::Key::Named(named) => map_named_key(named)?,
		keyboard::Key::Unidentified => return None,
	};

	Some(Key { code, modifiers })
}

fn map_mouse_button(button: mouse::Button) -> Option<CoreMouseButton> {
	match button {
		mouse::Button::Left => Some(CoreMouseButton::Left),
		mouse::Button::Right => Some(CoreMouseButton::Right),
		mouse::Button::Middle => Some(CoreMouseButton::Middle),
		mouse::Button::Back | mouse::Button::Forward | mouse::Button::Other(_) => None,
	}
}

fn map_scroll_delta(delta: mouse::ScrollDelta) -> Option<ScrollDirection> {
	let (x, y) = match delta {
		mouse::ScrollDelta::Lines { x, y } | mouse::ScrollDelta::Pixels { x, y } => (x, y),
	};

	if y.abs() >= x.abs() && y != 0.0 {
		return Some(if y > 0.0 { ScrollDirection::Up } else { ScrollDirection::Down });
	}

	if x != 0.0 {
		return Some(if x > 0.0 { ScrollDirection::Right } else { ScrollDirection::Left });
	}

	None
}

fn map_named_key(key: keyboard::key::Named) -> Option<KeyCode> {
	use keyboard::key::Named;

	match key {
		Named::ArrowDown => Some(KeyCode::Down),
		Named::ArrowLeft => Some(KeyCode::Left),
		Named::ArrowRight => Some(KeyCode::Right),
		Named::ArrowUp => Some(KeyCode::Up),
		Named::Backspace => Some(KeyCode::Backspace),
		Named::Delete => Some(KeyCode::Delete),
		Named::End => Some(KeyCode::End),
		Named::Enter => Some(KeyCode::Enter),
		Named::Escape => Some(KeyCode::Esc),
		Named::Home => Some(KeyCode::Home),
		Named::Insert => Some(KeyCode::Insert),
		Named::PageDown => Some(KeyCode::PageDown),
		Named::PageUp => Some(KeyCode::PageUp),
		Named::Space => Some(KeyCode::Space),
		Named::Tab => Some(KeyCode::Tab),
		Named::F1 => Some(KeyCode::F(1)),
		Named::F2 => Some(KeyCode::F(2)),
		Named::F3 => Some(KeyCode::F(3)),
		Named::F4 => Some(KeyCode::F(4)),
		Named::F5 => Some(KeyCode::F(5)),
		Named::F6 => Some(KeyCode::F(6)),
		Named::F7 => Some(KeyCode::F(7)),
		Named::F8 => Some(KeyCode::F(8)),
		Named::F9 => Some(KeyCode::F(9)),
		Named::F10 => Some(KeyCode::F(10)),
		Named::F11 => Some(KeyCode::F(11)),
		Named::F12 => Some(KeyCode::F(12)),
		Named::F13 => Some(KeyCode::F(13)),
		Named::F14 => Some(KeyCode::F(14)),
		Named::F15 => Some(KeyCode::F(15)),
		Named::F16 => Some(KeyCode::F(16)),
		Named::F17 => Some(KeyCode::F(17)),
		Named::F18 => Some(KeyCode::F(18)),
		Named::F19 => Some(KeyCode::F(19)),
		Named::F20 => Some(KeyCode::F(20)),
		Named::F21 => Some(KeyCode::F(21)),
		Named::F22 => Some(KeyCode::F(22)),
		Named::F23 => Some(KeyCode::F(23)),
		Named::F24 => Some(KeyCode::F(24)),
		Named::F25 => Some(KeyCode::F(25)),
		Named::F26 => Some(KeyCode::F(26)),
		Named::F27 => Some(KeyCode::F(27)),
		Named::F28 => Some(KeyCode::F(28)),
		Named::F29 => Some(KeyCode::F(29)),
		Named::F30 => Some(KeyCode::F(30)),
		Named::F31 => Some(KeyCode::F(31)),
		Named::F32 => Some(KeyCode::F(32)),
		Named::F33 => Some(KeyCode::F(33)),
		Named::F34 => Some(KeyCode::F(34)),
		Named::F35 => Some(KeyCode::F(35)),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn logical_pixels_to_cells_uses_floor_mapping() {
		assert_eq!(logical_pixels_to_cells(79.9, 8.0), 9);
		assert_eq!(logical_pixels_to_cells(80.0, 8.0), 10);
	}

	#[test]
	fn logical_pixels_to_cells_clamps_minimum_to_one_cell() {
		assert_eq!(logical_pixels_to_cells(0.0, 8.0), 1);
		assert_eq!(logical_pixels_to_cells(-10.0, 8.0), 1);
	}

	#[test]
	fn logical_pixels_to_cell_index_is_zero_based() {
		assert_eq!(logical_pixels_to_cell_index(0.0, 8.0), 0);
		assert_eq!(logical_pixels_to_cell_index(7.9, 8.0), 0);
		assert_eq!(logical_pixels_to_cell_index(8.0, 8.0), 1);
		assert_eq!(logical_pixels_to_cell_index(15.9, 8.0), 1);
		assert_eq!(logical_pixels_to_cell_index(16.0, 8.0), 2);
	}

	#[test]
	fn parse_cell_size_falls_back_for_invalid_values() {
		assert_eq!(parse_cell_size(Some(String::from("abc")), 8.0), 8.0);
		assert_eq!(parse_cell_size(Some(String::from("0")), 8.0), 8.0);
		assert_eq!(parse_cell_size(Some(String::from("-4")), 8.0), 8.0);
		assert_eq!(parse_cell_size(None, 8.0), 8.0);
	}

	#[test]
	fn default_cell_metrics_track_text_defaults() {
		assert_eq!(DEFAULT_CELL_WIDTH_PX, DEFAULT_TEXT_SIZE_PX * DEFAULT_MONOSPACE_WIDTH_FACTOR);
		assert_eq!(DEFAULT_CELL_HEIGHT_PX, DEFAULT_TEXT_SIZE_PX * DEFAULT_LINE_HEIGHT_FACTOR);
	}

	#[test]
	fn map_scroll_delta_prefers_vertical_direction() {
		assert_eq!(map_scroll_delta(mouse::ScrollDelta::Lines { x: 1.0, y: -2.0 }), Some(ScrollDirection::Down));
		assert_eq!(map_scroll_delta(mouse::ScrollDelta::Pixels { x: -2.0, y: 1.0 }), Some(ScrollDirection::Left));
		assert_eq!(map_scroll_delta(mouse::ScrollDelta::Lines { x: 0.0, y: 0.0 }), None);
	}

	#[test]
	fn map_input_method_event_maps_commit_to_paste() {
		let mut state = EventBridgeState::default();
		assert_eq!(
			map_input_method_event(input_method::Event::Commit(String::from("hello")), &mut state),
			Some(RuntimeEvent::Paste(String::from("hello")))
		);
		assert_eq!(map_input_method_event(input_method::Event::Commit(String::new()), &mut state), None);
		assert_eq!(map_input_method_event(input_method::Event::Opened, &mut state), None);
	}

	#[test]
	fn map_input_method_event_tracks_preedit_state() {
		let mut state = EventBridgeState::default();
		assert_eq!(
			map_input_method_event(input_method::Event::Preedit(String::from("compose"), None), &mut state),
			None
		);
		assert_eq!(state.ime_preedit(), Some("compose"));

		assert_eq!(
			map_input_method_event(input_method::Event::Commit(String::from("x")), &mut state),
			Some(RuntimeEvent::Paste(String::from("x")))
		);
		assert_eq!(state.ime_preedit(), None);
	}

	#[test]
	fn map_event_routes_ime_commit_to_runtime_paste() {
		let mut state = EventBridgeState::default();
		let metrics = CellMetrics {
			width_px: 8.0,
			height_px: 16.0,
		};

		let event = Event::InputMethod(input_method::Event::Commit(String::from("ime-text")));
		assert_eq!(map_event(event, metrics, &mut state), Some(RuntimeEvent::Paste(String::from("ime-text"))));
		assert_eq!(state.ime_preedit(), None);
	}

	#[test]
	fn map_event_tracks_ime_preedit_without_runtime_event() {
		let mut state = EventBridgeState::default();
		let metrics = CellMetrics {
			width_px: 8.0,
			height_px: 16.0,
		};

		let event = Event::InputMethod(input_method::Event::Preedit(String::from("compose"), None));
		assert_eq!(map_event(event, metrics, &mut state), None);
		assert_eq!(state.ime_preedit(), Some("compose"));
	}

	#[test]
	fn map_event_maps_window_resize_to_grid_cells() {
		let mut state = EventBridgeState::default();
		let metrics = CellMetrics {
			width_px: 8.0,
			height_px: 16.0,
		};

		let event = Event::Window(window::Event::Resized(iced::Size::new(80.0, 48.0)));
		assert_eq!(map_event(event, metrics, &mut state), Some(RuntimeEvent::WindowResized { cols: 10, rows: 3 }));
	}

	#[test]
	fn map_event_maps_mouse_move_press_drag_sequence() {
		let mut state = EventBridgeState::default();
		let metrics = CellMetrics {
			width_px: 8.0,
			height_px: 16.0,
		};

		let move_event = Event::Mouse(mouse::Event::CursorMoved {
			position: iced::Point::new(16.0, 32.0),
		});
		assert_eq!(
			map_event(move_event, metrics, &mut state),
			Some(RuntimeEvent::Mouse(CoreMouseEvent::Move { row: 2, col: 2 }))
		);

		let press_event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
		assert_eq!(
			map_event(press_event, metrics, &mut state),
			Some(RuntimeEvent::Mouse(CoreMouseEvent::Press {
				button: CoreMouseButton::Left,
				row: 2,
				col: 2,
				modifiers: Modifiers::NONE,
			}))
		);

		let drag_event = Event::Mouse(mouse::Event::CursorMoved {
			position: iced::Point::new(24.0, 48.0),
		});
		assert_eq!(
			map_event(drag_event, metrics, &mut state),
			Some(RuntimeEvent::Mouse(CoreMouseEvent::Drag {
				button: CoreMouseButton::Left,
				row: 3,
				col: 3,
				modifiers: Modifiers::NONE,
			}))
		);
	}
}
