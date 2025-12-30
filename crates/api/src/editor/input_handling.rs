use evildoer_base::{Key, Position, Selection};
use evildoer_input::KeyResult;
use evildoer_manifest::{
	Mode, SplitKey, SplitKeyCode, SplitModifiers, SplitMouse, SplitMouseAction, SplitMouseButton,
};
use termina::event::{KeyCode, Modifiers};

use crate::buffer::BufferView;
use crate::editor::Editor;

enum ActionDispatch {
	Executed(bool),
	NotAction,
}

impl Editor {
	fn dispatch_action(&mut self, result: &KeyResult) -> ActionDispatch {
		use evildoer_manifest::find_action_by_id;

		match result {
			KeyResult::ActionById {
				id,
				count,
				extend,
				register,
			} => {
				let quit = if let Some(action) = find_action_by_id(*id) {
					self.execute_action(action.name, *count, *extend, *register)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				};
				ActionDispatch::Executed(quit)
			}
			KeyResult::ActionByIdWithChar {
				id,
				count,
				extend,
				register,
				char_arg,
			} => {
				let quit = if let Some(action) = find_action_by_id(*id) {
					self.execute_action_with_char(
						action.name,
						*count,
						*extend,
						*register,
						*char_arg,
					)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				};
				ActionDispatch::Executed(quit)
			}
			_ => ActionDispatch::NotAction,
		}
	}

	pub async fn handle_key(&mut self, key: termina::event::KeyEvent) -> bool {
		// UI global bindings (panels, focus, etc.)
		if self.ui.handle_global_key(&key) {
			if self.ui.take_wants_redraw() {
				self.needs_redraw = true;
			}
			return false;
		}

		if self.ui.focused_panel_id().is_some() {
			let mut ui = std::mem::take(&mut self.ui);
			let _ = ui.handle_focused_key(self, key);
			if ui.take_wants_redraw() {
				self.needs_redraw = true;
			}
			self.ui = ui;
			return false;
		}

		// If a panel is focused, route input to it
		if let BufferView::Panel(panel_id) = self.focused_view() {
			let is_terminal = self.is_terminal_focused();

			// Ctrl+w enters window mode - use first buffer's input handler (terminal panels only)
			if is_terminal
				&& key.code == KeyCode::Char('w')
				&& key.modifiers.contains(Modifiers::CONTROL)
			{
				if let Some(first_buffer_id) = self.layout.first_buffer()
					&& let Some(buffer) = self.buffers.get_buffer_mut(first_buffer_id)
				{
					buffer.input.set_mode(Mode::Window);
					self.needs_redraw = true;
				}
				return false;
			}

			// Escape releases focus back to the first text buffer
			if key.code == KeyCode::Escape {
				if let Some(first_buffer) = self.layout.first_buffer() {
					self.focus_buffer(first_buffer);
				}
				self.needs_redraw = true;
				return false;
			}

			// Check if we're in window mode (using first buffer's input handler, terminal panels only)
			if is_terminal && let Some(first_buffer_id) = self.layout.first_buffer() {
				let in_window_mode = self
					.buffers
					.get_buffer(first_buffer_id)
					.is_some_and(|b| matches!(b.input.mode(), Mode::Window));

				if in_window_mode {
					// Process window mode key through first buffer's input handler
					return self.handle_terminal_window_key(key, first_buffer_id).await;
				}
			}

			// Route all other keys to the panel
			if let Some(split_key) = convert_termina_key(&key) {
				let result = self.handle_panel_key(panel_id, split_key);
				if result.needs_redraw {
					self.needs_redraw = true;
				}
				if result.release_focus
					&& let Some(first_buffer) = self.layout.first_buffer()
				{
					self.focus_buffer(first_buffer);
				}
			}
			return false;
		}

		self.handle_key_active(key).await
	}

	pub(crate) async fn handle_key_active(&mut self, key: termina::event::KeyEvent) -> bool {
		use evildoer_manifest::{HookContext, HookEventData, emit_hook};

		let old_mode = self.mode();
		let key: Key = key.into();

		let result = self.buffer_mut().input.handle_key(key);

		if let ActionDispatch::Executed(quit) = self.dispatch_action(&result) {
			return quit;
		}

		match result {
			KeyResult::Pending { .. } => {
				self.needs_redraw = true;
				false
			}
			KeyResult::ModeChange(new_mode) => {
				let leaving_insert = !matches!(new_mode, Mode::Insert);
				if new_mode != old_mode {
					emit_hook(&HookContext::new(
						HookEventData::ModeChange {
							old_mode,
							new_mode: new_mode.clone(),
						},
						Some(&self.extensions),
					))
					.await;
				}
				if leaving_insert {
					self.buffer_mut().clear_insert_undo_active();
				}
				false
			}
			KeyResult::InsertChar(c) => {
				self.insert_text(&c.to_string());
				false
			}
			KeyResult::Consumed | KeyResult::Unhandled => false,
			KeyResult::Quit => true,
			KeyResult::MouseClick { row, col, extend } => {
				// Keyboard-triggered mouse events use screen coordinates relative to
				// the focused buffer's area. Translate them to view-local coordinates.
				let view_area = self.focused_view_area();
				let local_row = row.saturating_sub(view_area.y);
				let local_col = col.saturating_sub(view_area.x);
				self.handle_mouse_click_local(local_row, local_col, extend);
				false
			}
			KeyResult::MouseDrag { row, col } => {
				let view_area = self.focused_view_area();
				let local_row = row.saturating_sub(view_area.y);
				let local_col = col.saturating_sub(view_area.x);
				self.handle_mouse_drag_local(local_row, local_col);
				false
			}
			KeyResult::MouseScroll { direction, count } => {
				self.handle_mouse_scroll(direction, count);
				false
			}
			_ => unreachable!(),
		}
	}

	/// Handles window mode keys when a terminal is focused.
	async fn handle_terminal_window_key(
		&mut self,
		key: termina::event::KeyEvent,
		buffer_id: crate::buffer::BufferId,
	) -> bool {
		let key: Key = key.into();

		let result = {
			let Some(buffer) = self.buffers.get_buffer_mut(buffer_id) else {
				return false;
			};
			buffer.input.handle_key(key)
		};

		if let ActionDispatch::Executed(quit) = self.dispatch_action(&result) {
			return quit;
		}

		match result {
			KeyResult::Quit => true,
			KeyResult::ModeChange(_) | KeyResult::Consumed | KeyResult::Unhandled => {
				self.needs_redraw = true;
				false
			}
			_ => false,
		}
	}

	pub async fn handle_mouse(&mut self, mouse: termina::event::MouseEvent) -> bool {
		let width = self.window_width.unwrap_or(80);
		let height = self.window_height.unwrap_or(24);
		// Main area excludes status line (1 row)
		let main_height = height.saturating_sub(1);
		let main_area = evildoer_tui::layout::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};

		let mut ui = std::mem::take(&mut self.ui);
		let dock_layout = ui.compute_layout(main_area);

		if ui.handle_mouse(self, mouse, &dock_layout) {
			if ui.take_wants_redraw() {
				self.needs_redraw = true;
			}
			self.ui = ui;
			return false;
		}
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;

		// Get the document area (excluding panels/docks)
		let doc_area = dock_layout.doc_area;

		self.handle_mouse_in_doc_area(mouse, doc_area).await
	}

	/// Handles mouse events within the document area (where splits live).
	///
	/// This method:
	/// 1. Handles active separator drag (resize) operations
	/// 2. Checks if mouse is over a separator (for hover/resize feedback)
	/// 3. Determines which view the mouse is over
	/// 4. Focuses that view if it's different from the current focus
	/// 5. Translates screen coordinates to view-local coordinates
	/// 6. Dispatches the mouse event to the appropriate handler
	///
	/// Text selection drags are confined to the view where they started.
	/// This prevents selection from crossing split boundaries.
	pub(crate) async fn handle_mouse_in_doc_area(
		&mut self,
		mouse: termina::event::MouseEvent,
		doc_area: evildoer_tui::layout::Rect,
	) -> bool {
		use termina::event::MouseEventKind;

		let mouse_x = mouse.column;
		let mouse_y = mouse.row;

		if let Some(drag_state) = self.layout.drag_state().cloned() {
			match mouse.kind {
				MouseEventKind::Drag(_) => {
					self.layout
						.resize_separator(doc_area, &drag_state.id, mouse_x, mouse_y);
					self.needs_redraw = true;
					return false;
				}
				MouseEventKind::Up(_) => {
					self.layout.end_drag();
					self.needs_redraw = true;
					return false;
				}
				_ => {}
			}
		}

		// Handle active text selection drag - confine to origin view
		if let Some((origin_view, origin_area)) = self.layout.text_selection_origin {
			match mouse.kind {
				MouseEventKind::Drag(_) => {
					let clamped_x =
						mouse_x.clamp(origin_area.x, origin_area.right().saturating_sub(1));
					let clamped_y =
						mouse_y.clamp(origin_area.y, origin_area.bottom().saturating_sub(1));
					let local_row = clamped_y.saturating_sub(origin_area.y);
					let local_col = clamped_x.saturating_sub(origin_area.x);

					if let BufferView::Text(buffer_id) = origin_view
						&& let Some(buffer) = self.buffers.get_buffer_mut(buffer_id)
					{
						let _ = buffer.input.handle_mouse(mouse.into());
						let doc_pos =
							buffer
								.screen_to_doc_position(local_row, local_col)
								.or_else(|| {
									let gutter_width = buffer.gutter_width();
									(local_col < gutter_width)
										.then(|| {
											buffer.screen_to_doc_position(local_row, gutter_width)
										})
										.flatten()
								});

						if let Some(doc_pos) = doc_pos {
							let anchor = buffer.selection.primary().anchor;
							buffer.selection = Selection::single(anchor, doc_pos);
							buffer.cursor = buffer.selection.primary().head;
						}
					}
					self.needs_redraw = true;
					return false;
				}
				MouseEventKind::Up(_) => {
					self.layout.text_selection_origin = None;
					self.needs_redraw = true;
				}
				_ => {}
			}
		}

		let separator_hit = self
			.layout
			.separator_hit_at_position(doc_area, mouse_x, mouse_y);

		self.layout.update_mouse_velocity(mouse_x, mouse_y);
		let is_fast_mouse = self.layout.is_mouse_fast();

		let current_separator = separator_hit.as_ref().map(|hit| (hit.direction, hit.rect));
		self.layout.separator_under_mouse = current_separator;

		match mouse.kind {
			MouseEventKind::Moved => {
				let old_hover = self.layout.hovered_separator;

				// Hover activation: sticky once active, velocity-gated for new hovers
				self.layout.hovered_separator = match (old_hover, current_separator) {
					(Some(old), Some(new)) if old == new => Some(old),
					(_, Some(sep)) if !is_fast_mouse => Some(sep),
					(_, Some(_)) => {
						self.needs_redraw = true;
						None
					}
					(_, None) => None,
				};

				if old_hover != self.layout.hovered_separator {
					self.layout
						.update_hover_animation(old_hover, self.layout.hovered_separator);
					self.needs_redraw = true;
				}

				if self.layout.hovered_separator.is_some() {
					return false;
				}
			}
			MouseEventKind::Down(_) => {
				if let Some(hit) = &separator_hit {
					self.layout.start_drag(hit);
					self.needs_redraw = true;
					return false;
				}
				if self.layout.hovered_separator.is_some() {
					let old_hover = self.layout.hovered_separator.take();
					self.layout.update_hover_animation(old_hover, None);
					self.needs_redraw = true;
				}
			}
			MouseEventKind::Drag(_) => {
				if self.layout.hovered_separator.is_some() {
					let old_hover = self.layout.hovered_separator.take();
					self.layout.update_hover_animation(old_hover, None);
					self.needs_redraw = true;
				}
			}
			_ => {
				if separator_hit.is_none() && self.layout.hovered_separator.is_some() {
					let old_hover = self.layout.hovered_separator.take();
					self.layout.update_hover_animation(old_hover, None);
					self.needs_redraw = true;
				}
			}
		}

		let Some((target_view, view_area)) =
			self.layout.view_at_position(doc_area, mouse_x, mouse_y)
		else {
			return false;
		};

		if target_view != self.focused_view() {
			let focus_changed = match mouse.kind {
				MouseEventKind::Down(_) => self.focus_view(target_view),
				_ => self.focus_view_implicit(target_view),
			};
			if !focus_changed && target_view != self.focused_view() {
				return false;
			}
		}

		if let BufferView::Panel(panel_id) = self.focused_view() {
			let local_x = mouse_x.saturating_sub(view_area.x);
			let local_y = mouse_y.saturating_sub(view_area.y);

			if let Some(split_mouse) = convert_mouse_event(&mouse, local_x, local_y) {
				let result = self.handle_panel_mouse(panel_id, split_mouse);
				if result.needs_redraw {
					self.needs_redraw = true;
				}
			}
			return false;
		}

		// Translate screen coordinates to view-local coordinates
		let local_row = mouse_y.saturating_sub(view_area.y);
		let local_col = mouse_x.saturating_sub(view_area.x);

		// Process the mouse event through the input handler
		let result = self.buffer_mut().input.handle_mouse(mouse.into());
		match result {
			KeyResult::MouseClick { extend, .. } => {
				self.layout.text_selection_origin = Some((target_view, view_area));
				self.handle_mouse_click_local(local_row, local_col, extend);
				false
			}
			KeyResult::MouseDrag { .. } => {
				self.handle_mouse_drag_local(local_row, local_col);
				false
			}
			KeyResult::MouseScroll { direction, count } => {
				self.handle_mouse_scroll(direction, count);
				false
			}
			_ => false,
		}
	}

	/// Handles a mouse click with view-local coordinates.
	pub(crate) fn handle_mouse_click_local(
		&mut self,
		local_row: u16,
		local_col: u16,
		extend: bool,
	) {
		if let Some(doc_pos) = self.buffer().screen_to_doc_position(local_row, local_col) {
			let buffer = self.buffer_mut();
			if extend {
				let anchor = buffer.selection.primary().anchor;
				buffer.selection = Selection::single(anchor, doc_pos);
			} else {
				buffer.selection = Selection::point(doc_pos);
			}
			buffer.cursor = buffer.selection.primary().head;
		}
	}

	/// Handles mouse drag with view-local coordinates.
	pub(crate) fn handle_mouse_drag_local(&mut self, local_row: u16, local_col: u16) {
		if let Some(doc_pos) = self.buffer().screen_to_doc_position(local_row, local_col) {
			let buffer = self.buffer_mut();
			let anchor = buffer.selection.primary().anchor;
			buffer.selection = Selection::single(anchor, doc_pos);
			buffer.cursor = buffer.selection.primary().head;
		}
	}

	/// Returns the screen area of the currently focused view.
	///
	/// This computes the document area (excluding status line and panels)
	/// and then finds the focused view's rectangle within that area.
	fn focused_view_area(&self) -> evildoer_tui::layout::Rect {
		let doc_area = self.doc_area();
		let focused = self.focused_view();
		for (view, area) in self.layout.compute_view_areas(doc_area) {
			if view == focused {
				return area;
			}
		}
		doc_area
	}

	/// Computes the document area based on current window dimensions.
	pub fn doc_area(&self) -> evildoer_tui::layout::Rect {
		let width = self.window_width.unwrap_or(80);
		let height = self.window_height.unwrap_or(24);
		let main_height = height.saturating_sub(1);
		let main_area = evildoer_tui::layout::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};
		self.ui.compute_layout(main_area).doc_area
	}

	/// Handles a key event for a panel.
	fn handle_panel_key(
		&mut self,
		panel_id: evildoer_manifest::PanelId,
		key: SplitKey,
	) -> evildoer_manifest::SplitEventResult {
		if let Some(panel) = self.panels.get_mut(panel_id) {
			panel.handle_key(key)
		} else {
			evildoer_manifest::SplitEventResult::ignored()
		}
	}

	/// Handles a mouse event for a panel.
	fn handle_panel_mouse(
		&mut self,
		panel_id: evildoer_manifest::PanelId,
		mouse: SplitMouse,
	) -> evildoer_manifest::SplitEventResult {
		if let Some(panel) = self.panels.get_mut(panel_id) {
			panel.handle_mouse(mouse)
		} else {
			evildoer_manifest::SplitEventResult::ignored()
		}
	}
}

/// Converts a termina KeyEvent to a SplitKey for terminal input.
fn convert_termina_key(key: &termina::event::KeyEvent) -> Option<SplitKey> {
	let code = match key.code {
		KeyCode::Char(c) => SplitKeyCode::Char(c),
		KeyCode::Enter => SplitKeyCode::Enter,
		KeyCode::Escape => SplitKeyCode::Escape,
		KeyCode::Backspace => SplitKeyCode::Backspace,
		KeyCode::Tab => SplitKeyCode::Tab,
		KeyCode::Up => SplitKeyCode::Up,
		KeyCode::Down => SplitKeyCode::Down,
		KeyCode::Left => SplitKeyCode::Left,
		KeyCode::Right => SplitKeyCode::Right,
		KeyCode::Home => SplitKeyCode::Home,
		KeyCode::End => SplitKeyCode::End,
		KeyCode::PageUp => SplitKeyCode::PageUp,
		KeyCode::PageDown => SplitKeyCode::PageDown,
		KeyCode::Delete => SplitKeyCode::Delete,
		KeyCode::Insert => SplitKeyCode::Insert,
		_ => return None,
	};

	let mut modifiers = SplitModifiers::NONE;
	if key.modifiers.contains(Modifiers::CONTROL) {
		modifiers = modifiers.union(SplitModifiers::CTRL);
	}
	if key.modifiers.contains(Modifiers::ALT) {
		modifiers = modifiers.union(SplitModifiers::ALT);
	}
	if key.modifiers.contains(Modifiers::SHIFT) {
		modifiers = modifiers.union(SplitModifiers::SHIFT);
	}

	Some(SplitKey::new(code, modifiers))
}

fn convert_mouse_event(
	mouse: &termina::event::MouseEvent,
	local_x: u16,
	local_y: u16,
) -> Option<SplitMouse> {
	use termina::event::{MouseButton, MouseEventKind};

	let btn = |b: MouseButton| match b {
		MouseButton::Left => SplitMouseButton::Left,
		MouseButton::Right => SplitMouseButton::Right,
		MouseButton::Middle => SplitMouseButton::Middle,
	};

	let action = match mouse.kind {
		MouseEventKind::Down(b) => SplitMouseAction::Press(btn(b)),
		MouseEventKind::Up(b) => SplitMouseAction::Release(btn(b)),
		MouseEventKind::Drag(b) => SplitMouseAction::Drag(btn(b)),
		MouseEventKind::ScrollUp => SplitMouseAction::ScrollUp,
		MouseEventKind::ScrollDown => SplitMouseAction::ScrollDown,
		_ => return None,
	};

	Some(SplitMouse {
		position: Position::new(local_x, local_y),
		action,
	})
}
