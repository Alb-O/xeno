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
fn map_event_ignores_window_resize_events() {
	let mut state = EventBridgeState::default();
	let metrics = CellMetrics {
		width_px: 8.0,
		height_px: 16.0,
	};

	let event = Event::Window(window::Event::Resized(iced::Size::new(80.0, 48.0)));
	assert_eq!(map_event(event, metrics, &mut state), None);
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

#[test]
fn map_key_event_named_space_produces_space_keycode() {
	let key = map_key_event(
		keyboard::Key::Named(keyboard::key::Named::Space),
		keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
		keyboard::Modifiers::empty(),
	);
	assert_eq!(key.unwrap().code, KeyCode::Space);
}

#[test]
fn map_key_event_character_space_canonicalizes_to_space_keycode() {
	let key = map_key_event(
		keyboard::Key::Character(" ".into()),
		keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
		keyboard::Modifiers::empty(),
	);
	assert_eq!(key.unwrap().code, KeyCode::Space);
}

#[test]
fn map_key_event_logo_modifier_sets_cmd() {
	let key = map_key_event(
		keyboard::Key::Character("a".into()),
		keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
		keyboard::Modifiers::LOGO,
	);
	let key = key.unwrap();
	assert!(key.modifiers.cmd, "LOGO modifier should map to cmd=true");
}

#[test]
fn map_key_event_f35_maps_correctly() {
	let key = map_key_event(
		keyboard::Key::Named(keyboard::key::Named::F35),
		keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
		keyboard::Modifiers::empty(),
	);
	assert_eq!(key.unwrap().code, KeyCode::F(35));
}

#[test]
fn map_key_event_multi_char_returns_none() {
	let key = map_key_event(
		keyboard::Key::Character("ab".into()),
		keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
		keyboard::Modifiers::empty(),
	);
	assert!(key.is_none());
}
