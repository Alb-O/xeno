use termina::event::{KeyCode, Modifiers};
use tome_base::{Key, Selection};
use tome_input::KeyResult;
use tome_manifest::{Mode, SplitBuffer, SplitKey, SplitKeyCode, SplitModifiers};

use crate::buffer::{BufferView, SplitDirection};
use crate::editor::{DragState, Editor, SeparatorHoverAnimation};

impl Editor {
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

		// If a terminal is focused, route input to it
		// Exception: Ctrl+w enters window mode, Escape releases focus
		if let BufferView::Terminal(terminal_id) = self.focused_view() {
			// Ctrl+w enters window mode - use first buffer's input handler
			if key.code == KeyCode::Char('w') && key.modifiers.contains(Modifiers::CONTROL) {
				if let Some(first_buffer_id) = self.layout.first_buffer()
					&& let Some(buffer) = self.buffers.get_mut(&first_buffer_id)
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

			// Check if we're in window mode (using first buffer's input handler)
			if let Some(first_buffer_id) = self.layout.first_buffer() {
				let in_window_mode = self
					.buffers
					.get(&first_buffer_id)
					.is_some_and(|b| matches!(b.input.mode(), Mode::Window));

				if in_window_mode {
					// Process window mode key through first buffer's input handler
					return self.handle_terminal_window_key(key, first_buffer_id).await;
				}
			}

			// Route all other keys to the terminal
			if let Some(split_key) = convert_termina_key(&key)
				&& let Some(terminal) = self.terminals.get_mut(&terminal_id)
			{
				let result = terminal.handle_key(split_key);
				if result.needs_redraw {
					self.needs_redraw = true;
				}
			}
			return false;
		}

		self.handle_key_active(key).await
	}

	pub(crate) async fn handle_key_active(&mut self, key: termina::event::KeyEvent) -> bool {
		use tome_manifest::{HookContext, emit_hook, find_action_by_id};

		self.message = None;

		let old_mode = self.mode();
		let key: Key = key.into();

		let result = self.buffer_mut().input.handle_key(key);

		match result {
			// Typed ActionId dispatch (preferred path)
			KeyResult::ActionById {
				id,
				count,
				extend,
				register,
			} => {
				if let Some(action) = find_action_by_id(id) {
					self.execute_action(action.name, count, extend, register)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				}
			}
			KeyResult::ActionByIdWithChar {
				id,
				count,
				extend,
				register,
				char_arg,
			} => {
				if let Some(action) = find_action_by_id(id) {
					self.execute_action_with_char(action.name, count, extend, register, char_arg)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				}
			}
			// String-based dispatch (backward compatibility)
			KeyResult::Action {
				name,
				count,
				extend,
				register,
			} => self.execute_action(name, count, extend, register),
			KeyResult::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => self.execute_action_with_char(name, count, extend, register, char_arg),
			KeyResult::ModeChange(new_mode) => {
				let is_normal = matches!(new_mode, Mode::Normal);
				let leaving_insert = !matches!(new_mode, Mode::Insert);
				if new_mode != old_mode {
					emit_hook(&HookContext::ModeChange {
						old_mode,
						new_mode: new_mode.clone(),
					});
				}
				if is_normal {
					self.message = None;
				}
				if leaving_insert {
					self.buffer_mut().insert_undo_active = false;
				}
				false
			}
			KeyResult::InsertChar(c) => {
				self.insert_text(&c.to_string());
				false
			}
			KeyResult::Consumed => false,
			KeyResult::Unhandled => false,
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
		}
	}

	/// Handles window mode keys when a terminal is focused.
	///
	/// Uses the specified buffer's input handler for window mode processing.
	async fn handle_terminal_window_key(
		&mut self,
		key: termina::event::KeyEvent,
		buffer_id: crate::buffer::BufferId,
	) -> bool {
		use tome_manifest::find_action_by_id;

		let key: Key = key.into();

		// Get the result from the buffer's input handler
		let result = {
			let Some(buffer) = self.buffers.get_mut(&buffer_id) else {
				return false;
			};
			buffer.input.handle_key(key)
		};

		match result {
			KeyResult::ActionById {
				id,
				count,
				extend,
				register,
			} => {
				if let Some(action) = find_action_by_id(id) {
					self.execute_action(action.name, count, extend, register)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				}
			}
			KeyResult::ActionByIdWithChar {
				id,
				count,
				extend,
				register,
				char_arg,
			} => {
				if let Some(action) = find_action_by_id(id) {
					self.execute_action_with_char(action.name, count, extend, register, char_arg)
				} else {
					self.notify("error", format!("Unknown action ID: {}", id));
					false
				}
			}
			KeyResult::Action {
				name,
				count,
				extend,
				register,
			} => self.execute_action(name, count, extend, register),
			KeyResult::ActionWithChar {
				name,
				count,
				extend,
				register,
				char_arg,
			} => self.execute_action_with_char(name, count, extend, register, char_arg),
			KeyResult::ModeChange(_) | KeyResult::Consumed | KeyResult::Unhandled => {
				self.needs_redraw = true;
				false
			}
			KeyResult::Quit => true,
			// These shouldn't happen in window mode but handle gracefully
			KeyResult::InsertChar(_)
			| KeyResult::MouseClick { .. }
			| KeyResult::MouseDrag { .. }
			| KeyResult::MouseScroll { .. } => false,
		}
	}

	pub async fn handle_mouse(&mut self, mouse: termina::event::MouseEvent) -> bool {
		let width = self.window_width.unwrap_or(80);
		let height = self.window_height.unwrap_or(24);
		// Main area excludes status line (1 row)
		let main_height = height.saturating_sub(1);
		let main_area = tome_tui::layout::Rect {
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
	pub(crate) async fn handle_mouse_in_doc_area(
		&mut self,
		mouse: termina::event::MouseEvent,
		doc_area: tome_tui::layout::Rect,
	) -> bool {
		use termina::event::MouseEventKind;

		let mouse_x = mouse.column;
		let mouse_y = mouse.row;

		// Handle active drag operation
		if let Some(ref drag_state) = self.dragging_separator {
			match mouse.kind {
				MouseEventKind::Drag(_) => {
					// Continue resizing - use path to identify the split
					let path = drag_state.path.clone();
					self.layout
						.resize_at_path(doc_area, &path, mouse_x, mouse_y);
					self.needs_redraw = true;
					return false;
				}
				MouseEventKind::Up(_) => {
					// End drag operation
					self.dragging_separator = None;
					self.hovered_separator = None;
					self.needs_redraw = true;
					return false;
				}
				_ => {}
			}
		}

		// Check if mouse is over a separator
		let separator_info = self
			.layout
			.separator_with_path_at_position(doc_area, mouse_x, mouse_y);

		// Update mouse velocity tracker
		self.mouse_velocity.update(mouse_x, mouse_y);
		let is_fast_mouse = self.mouse_velocity.is_fast();

		// Always track which separator the mouse is physically over
		let current_separator = separator_info
			.as_ref()
			.map(|(direction, sep_rect, _)| (*direction, *sep_rect));
		self.separator_under_mouse = current_separator;

		// Update hover/drag state based on mouse event type
		match mouse.kind {
			MouseEventKind::Moved => {
				let old_hover = self.hovered_separator;

				// Determine new hover state:
				// - If already hovering a separator and mouse is still over it, keep it
				//   (regardless of velocity - once active, stay active while over it)
				// - If not hovering, only start hover if velocity is slow
				// - If mouse left the separator, clear hover
				self.hovered_separator = match (old_hover, current_separator) {
					// Already hovering same separator - keep it active
					(Some(old), Some(new)) if old == new => Some(old),
					// Mouse moved to different separator - only activate if slow
					(_, Some(sep)) if !is_fast_mouse => Some(sep),
					// Mouse over separator but moving fast and wasn't already hovering it
					(_, Some(_)) => {
						// Request redraw to check velocity again soon
						self.needs_redraw = true;
						None
					}
					// Mouse not over any separator
					(_, None) => None,
				};

				// Trigger animation if hover state changed
				if old_hover != self.hovered_separator {
					self.update_separator_hover_animation(old_hover, self.hovered_separator);
					self.needs_redraw = true;
				}

				// Mouse move over separator doesn't need further processing
				if self.hovered_separator.is_some() {
					return false;
				}
			}
			MouseEventKind::Down(_) => {
				// Start drag if clicking on a separator
				if let Some((direction, separator_rect, path)) = separator_info {
					self.dragging_separator = Some(DragState { direction, path });
					let old_hover = self.hovered_separator.take();
					self.hovered_separator = Some((direction, separator_rect));
					if old_hover != self.hovered_separator {
						self.update_separator_hover_animation(old_hover, self.hovered_separator);
					}
					self.needs_redraw = true;
					return false;
				}
				// Clear hover when clicking elsewhere
				if self.hovered_separator.is_some() {
					let old_hover = self.hovered_separator.take();
					self.update_separator_hover_animation(old_hover, None);
					self.needs_redraw = true;
				}
			}
			MouseEventKind::Drag(_) => {
				// Clear hover when dragging (not on separator)
				if self.hovered_separator.is_some() {
					let old_hover = self.hovered_separator.take();
					self.update_separator_hover_animation(old_hover, None);
					self.needs_redraw = true;
				}
			}
			_ => {
				// For other events (scroll, release), clear hover if not on separator
				if separator_info.is_none() && self.hovered_separator.is_some() {
					let old_hover = self.hovered_separator.take();
					self.update_separator_hover_animation(old_hover, None);
					self.needs_redraw = true;
				}
			}
		}

		// Find which view the mouse is over
		let Some((target_view, view_area)) =
			self.layout.view_at_position(doc_area, mouse_x, mouse_y)
		else {
			// Mouse is outside all views (e.g., on a separator)
			return false;
		};

		// Focus the target view if different from current
		if target_view != self.focused_view() {
			self.focus_view(target_view);
		}

		// Terminal views don't handle mouse events through the input system
		if self.is_terminal_focused() {
			return false;
		}

		// Translate screen coordinates to view-local coordinates
		let local_row = mouse_y.saturating_sub(view_area.y);
		let local_col = mouse_x.saturating_sub(view_area.x);

		// Process the mouse event through the input handler
		let result = self.buffer_mut().input.handle_mouse(mouse.into());
		match result {
			KeyResult::MouseClick { extend, .. } => {
				// Use local coordinates instead of the ones from KeyResult
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
	fn focused_view_area(&self) -> tome_tui::layout::Rect {
		let width = self.window_width.unwrap_or(80);
		let height = self.window_height.unwrap_or(24);
		let main_height = height.saturating_sub(1);
		let main_area = tome_tui::layout::Rect {
			x: 0,
			y: 0,
			width,
			height: main_height,
		};

		// Compute dock layout to get doc_area
		let dock_layout = self.ui.compute_layout(main_area);
		let doc_area = dock_layout.doc_area;

		// Find the focused view's area within the layout
		let focused = self.focused_view();
		for (view, area) in self.layout.compute_view_areas(doc_area) {
			if view == focused {
				return area;
			}
		}

		// Fallback to entire doc area if view not found
		doc_area
	}

	/// Updates the separator hover animation when hover state changes.
	pub fn update_separator_hover_animation(
		&mut self,
		old: Option<(SplitDirection, tome_tui::layout::Rect)>,
		new: Option<(SplitDirection, tome_tui::layout::Rect)>,
	) {
		match (old, new) {
			(None, Some((_, rect))) => {
				// Started hovering - animate in
				self.separator_hover_animation = Some(SeparatorHoverAnimation::new(rect, true));
			}
			(Some((_, old_rect)), None) => {
				// Stopped hovering - animate out from current position
				// If we have an existing animation for this rect, toggle it off
				// Otherwise create a new one starting from fully hovered
				if let Some(ref mut anim) = self.separator_hover_animation
					&& anim.rect == old_rect
				{
					// Same separator - just toggle the existing animation
					anim.set_hovering(false);
					return;
				}
				// Different separator or no existing animation - create new one at full intensity
				self.separator_hover_animation = Some(SeparatorHoverAnimation::new_at_intensity(
					old_rect, 1.0, false,
				));
			}
			(Some((_, old_rect)), Some((_, new_rect))) if old_rect != new_rect => {
				// Moved to a different separator - start fresh animation
				self.separator_hover_animation = Some(SeparatorHoverAnimation::new(new_rect, true));
			}
			_ => {
				// Same separator or both None - no change needed
			}
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
