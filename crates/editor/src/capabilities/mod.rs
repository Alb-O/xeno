//! Implementation of [`EditorCapabilities`] for [`Editor`].
//!
//! [`EditorCapabilities`]: xeno_registry::EditorCapabilities

use std::path::PathBuf;

use xeno_primitives::direction::{Axis, SeqDirection, SpatialDirection};
use xeno_primitives::range::{CharIdx, Direction as MoveDir};
use xeno_primitives::{BoxFutureLocal, Mode, Range, Selection};
use xeno_registry::actions::editor_ctx::{OverlayAccess, OverlayRequest};
use xeno_registry::commands::{CommandEditorOps, CommandError};
use xeno_registry::notifications::{Notification, keys};
use xeno_registry::options::{OptionKey, OptionScope, OptionValue, find_by_kdl, parse};
use xeno_registry::{
	CommandQueueAccess, CursorAccess, EditAccess, EditorCapabilities, FileOpsAccess, FocusOps,
	HookContext, HookEventData, JumpAccess, MacroAccess, ModeAccess, MotionAccess,
	MotionDispatchAccess, MotionKind, MotionRequest, NotificationAccess, OptionAccess,
	PaletteAccess, SearchAccess, SelectionAccess, SplitOps, ThemeAccess, UndoAccess,
	ViewportAccess, emit_sync_with as emit_hook_sync_with, motions,
};

use crate::impls::Editor;
use crate::overlay::LayerEvent;

/// Parses a string value into an [`OptionValue`] based on the option's declared type.
///
/// Uses centralized validation from the options registry, including type checking
/// and any custom validators defined on the option.
fn parse_option_value(kdl_key: &str, value: &str) -> Result<OptionValue, CommandError> {
	use xeno_registry::options::OptionError;

	parse::parse_value(kdl_key, value).map_err(|e| match e {
		OptionError::UnknownOption(key) => {
			let suggestion = parse::suggest_option(&key);
			match suggestion {
				Some(s) => CommandError::InvalidArgument(format!(
					"unknown option: {key} (did you mean '{s}'?)"
				)),
				None => CommandError::InvalidArgument(format!("unknown option: {key}")),
			}
		}
		OptionError::InvalidValue { option, reason } => {
			CommandError::InvalidArgument(format!("invalid value for {option}: {reason}"))
		}
		OptionError::TypeMismatch {
			option,
			expected,
			got,
		} => CommandError::InvalidArgument(format!(
			"type mismatch for {option}: expected {expected:?}, got {got}"
		)),
	})
}

impl CursorAccess for Editor {
	fn focused_view(&self) -> xeno_registry::hooks::ViewId {
		self.focused_view()
	}

	fn cursor(&self) -> CharIdx {
		self.buffer().cursor
	}

	fn cursor_line_col(&self) -> Option<(usize, usize)> {
		let buffer = self.buffer();
		Some((buffer.cursor_line(), buffer.cursor_col()))
	}

	fn set_cursor(&mut self, pos: CharIdx) {
		let view = self.focused_view();
		self.buffer_mut().set_cursor(pos);
		self.state
			.effects
			.push_layer_event(LayerEvent::CursorMoved { view });
	}
}

impl SelectionAccess for Editor {
	fn selection(&self) -> &Selection {
		&self.buffer().selection
	}

	fn selection_mut(&mut self) -> &mut Selection {
		&mut self.buffer_mut().selection
	}

	fn set_selection(&mut self, sel: Selection) {
		let view = self.focused_view();
		self.buffer_mut().set_selection(sel);
		self.state
			.effects
			.push_layer_event(LayerEvent::CursorMoved { view });
	}
}

impl ModeAccess for Editor {
	fn mode(&self) -> Mode {
		self.buffer().input.mode()
	}

	fn set_mode(&mut self, mode: Mode) {
		if matches!(mode, Mode::Insert) && self.buffer().is_readonly() {
			NotificationAccess::emit(self, keys::BUFFER_READONLY.into());
			return;
		}
		#[cfg(feature = "lsp")]
		if matches!(mode, Mode::Insert) {
			self.overlays_mut()
				.get_or_default::<crate::CompletionState>()
				.suppressed = false;
		}
		let view = self.focused_view();
		self.buffer_mut().input.set_mode(mode.clone());
		self.state
			.effects
			.push_layer_event(LayerEvent::ModeChanged { view, mode });
	}
}

impl NotificationAccess for Editor {
	fn emit(&mut self, notification: Notification) {
		self.state.effects.notify(notification);
	}

	fn clear_notifications(&mut self) {
		self.clear_all_notifications();
		self.state.effects.request_redraw();
	}
}

impl SearchAccess for Editor {
	fn search(&mut self, direction: SeqDirection, add_selection: bool, extend: bool) -> bool {
		match direction {
			SeqDirection::Next => self.do_search_next(add_selection, extend),
			SeqDirection::Prev => self.do_search_prev(add_selection, extend),
		}
	}

	fn search_repeat(&mut self, flip: bool, add_selection: bool, extend: bool) -> bool {
		self.do_search_repeat(flip, add_selection, extend)
	}

	fn use_selection_as_pattern(&mut self) -> bool {
		self.do_use_selection_as_search()
	}

	fn pattern(&self) -> Option<&str> {
		self.buffer().input.last_search().map(|(p, _)| p)
	}

	fn set_pattern(&mut self, pattern: &str) {
		self.buffer_mut()
			.input
			.set_last_search(pattern.to_string(), false);
	}
}

impl UndoAccess for Editor {
	fn undo(&mut self) {
		self.undo();
	}

	fn redo(&mut self) {
		self.redo();
	}

	fn can_undo(&self) -> bool {
		self.state.core.undo_manager.can_undo()
	}

	fn can_redo(&self) -> bool {
		self.state.core.undo_manager.can_redo()
	}
}

impl EditAccess for Editor {
	fn execute_edit_op(&mut self, op: &xeno_registry::edit_op::EditOp) {
		self.execute_edit_op(op.clone());
	}

	fn paste(&mut self, before: bool) {
		if before {
			self.paste_before();
		} else {
			self.paste_after();
		}
	}
}

impl MotionAccess for Editor {
	fn move_visual_vertical(&mut self, direction: MoveDir, count: usize, extend: bool) {
		self.move_visual_vertical(direction, count, extend);
	}
}

impl MotionDispatchAccess for Editor {
	fn apply_motion(&mut self, req: &MotionRequest) -> Selection {
		let Some(motion_key) = motions::find(req.id.as_str()) else {
			tracing::warn!("unknown motion: {}", req.id.as_str());
			return self.selection().clone();
		};

		let handler = motion_key.def().handler;
		let selection = self.selection().clone();
		let is_normal = self.mode() == Mode::Normal;

		let MotionRequest {
			count,
			extend,
			kind,
			..
		} = *req;

		let new_ranges = self.buffer().with_doc(|doc| {
			let text = doc.content().slice(..);
			selection
				.ranges()
				.iter()
				.map(|range| {
					let mut target = handler(text, *range, count, extend);

					if is_normal {
						target.head = xeno_primitives::rope::clamp_to_cell(target.head, text);
					}

					match kind {
						MotionKind::Cursor if extend => Range::new(range.anchor, target.head),
						MotionKind::Cursor => Range::point(target.head),
						MotionKind::Selection => Range::new(range.anchor, target.head),
						MotionKind::Word if extend => Range::new(range.anchor, target.head),
						MotionKind::Word => target,
					}
				})
				.collect::<Vec<_>>()
		});

		Selection::from_vec(new_ranges, selection.primary_index())
	}
}

impl ThemeAccess for Editor {
	fn set_theme(&mut self, name: &str) -> Result<(), CommandError> {
		Editor::set_theme(self, name)
	}
}

impl CommandEditorOps for Editor {
	fn emit(&mut self, notification: Notification) {
		self.state.effects.notify(notification);
	}

	fn clear_notifications(&mut self) {
		self.clear_all_notifications();
		self.state.effects.request_redraw();
	}

	fn is_modified(&self) -> bool {
		FileOpsAccess::is_modified(self)
	}

	fn is_readonly(&self) -> bool {
		self.buffer().is_readonly()
	}

	fn set_readonly(&mut self, readonly: bool) {
		self.buffer_mut().set_readonly(readonly);
	}

	fn save(&mut self) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		FileOpsAccess::save(self)
	}

	fn save_as(&mut self, path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		FileOpsAccess::save_as(self, path)
	}

	fn set_theme(&mut self, name: &str) -> Result<(), CommandError> {
		ThemeAccess::set_theme(self, name)
	}

	fn set_option(&mut self, kdl_key: &str, value: &str) -> Result<(), CommandError> {
		let opt_value = parse_option_value(kdl_key, value)?;
		let _ = self
			.state
			.config
			.global_options
			.set_by_kdl(kdl_key, opt_value);

		if let Some(def) = find_by_kdl(kdl_key) {
			emit_hook_sync_with(
				&HookContext::new(HookEventData::OptionChanged {
					key: def.kdl_key,
					scope: "global",
				}),
				&mut self.state.hook_runtime,
			);
		}
		Ok(())
	}

	fn set_local_option(&mut self, kdl_key: &str, value: &str) -> Result<(), CommandError> {
		let def = find_by_kdl(kdl_key).ok_or_else(|| {
			let suggestion = parse::suggest_option(kdl_key);
			CommandError::InvalidArgument(match suggestion {
				Some(s) => format!("unknown option '{kdl_key}'. Did you mean '{s}'?"),
				None => format!("unknown option '{kdl_key}'"),
			})
		})?;

		if def.scope == OptionScope::Global {
			return Err(CommandError::InvalidArgument(format!(
				"'{kdl_key}' is a global option, use :set instead of :setlocal"
			)));
		}

		let opt_value = parse_option_value(kdl_key, value)?;
		let _ = self
			.buffer_mut()
			.local_options
			.set_by_kdl(kdl_key, opt_value);

		emit_hook_sync_with(
			&HookContext::new(HookEventData::OptionChanged {
				key: def.kdl_key,
				scope: "buffer",
			}),
			&mut self.state.hook_runtime,
		);
		Ok(())
	}

	fn open_info_popup(&mut self, content: &str, _file_type: Option<&str>) {
		self.state
			.effects
			.overlay_request(OverlayRequest::ShowInfoPopup {
				title: None,
				body: content.to_string(),
			});
	}

	fn close_all_info_popups(&mut self) {
		// TODO: Add CloseInfoPopups to OverlayRequest if needed, or use CloseModal
	}

	fn goto_file(
		&mut self,
		path: PathBuf,
		line: usize,
		column: usize,
	) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move {
			use crate::impls::Location;
			self.goto_location(&Location::new(path, line, column))
				.await
				.map_err(|e| CommandError::Io(e.to_string()))?;
			Ok(())
		})
	}
}

impl From<crate::layout::SplitError> for xeno_registry::actions::editor_ctx::SplitError {
	fn from(e: crate::layout::SplitError) -> Self {
		match e {
			crate::layout::SplitError::ViewNotFound => Self::ViewNotFound,
			crate::layout::SplitError::AreaTooSmall => Self::AreaTooSmall,
		}
	}
}

impl SplitOps for Editor {
	fn split(&mut self, axis: Axis) -> Result<(), xeno_registry::actions::editor_ctx::SplitError> {
		match axis {
			Axis::Horizontal => Editor::split_horizontal_with_clone(self).map_err(Into::into),
			Axis::Vertical => Editor::split_vertical_with_clone(self).map_err(Into::into),
		}
	}

	fn close_split(&mut self) {
		self.close_current_buffer();
	}

	fn close_other_buffers(&mut self) {
		let current_id = self.focused_view();
		for id in self.buffer_ids() {
			if id != current_id {
				Editor::close_buffer(self, id);
			}
		}
	}
}

impl FocusOps for Editor {
	fn buffer_switch(&mut self, direction: SeqDirection) {
		match direction {
			SeqDirection::Next => self.focus_next_buffer(),
			SeqDirection::Prev => self.focus_prev_buffer(),
		}
	}

	fn focus(&mut self, direction: SpatialDirection) {
		self.focus_direction(direction);
	}
}

impl ViewportAccess for Editor {
	fn viewport_height(&self) -> usize {
		self.buffer().last_viewport_height
	}

	fn viewport_row_to_doc_position(&self, row: usize) -> Option<CharIdx> {
		let buffer = self.buffer();
		if buffer.last_viewport_height == 0 {
			return None;
		}
		let tab_width = self.tab_width();
		buffer
			.screen_to_doc_position(row as u16, buffer.gutter_width(), tab_width)
			.map(|pos| pos as CharIdx)
	}
}

impl JumpAccess for Editor {
	fn jump_forward(&mut self) -> bool {
		if let Some(loc) = self.state.core.workspace.jump_list.jump_forward() {
			let buffer_id = loc.buffer_id;
			let cursor = loc.cursor;
			if self.focused_view() != buffer_id {
				self.focus_buffer(buffer_id);
			}
			self.buffer_mut().set_cursor(cursor);
			self.state
				.effects
				.push_layer_event(crate::overlay::LayerEvent::CursorMoved { view: buffer_id });
			true
		} else {
			false
		}
	}

	fn jump_backward(&mut self) -> bool {
		let buffer_id = self.focused_view();
		let cursor = self.buffer().cursor;
		self.state
			.core
			.workspace
			.jump_list
			.push(crate::impls::JumpLocation { buffer_id, cursor });

		if let Some(loc) = self.state.core.workspace.jump_list.jump_backward() {
			let buffer_id = loc.buffer_id;
			let cursor = loc.cursor;
			if self.focused_view() != buffer_id {
				self.focus_buffer(buffer_id);
			}
			self.buffer_mut().set_cursor(cursor);
			self.state
				.effects
				.push_layer_event(crate::overlay::LayerEvent::CursorMoved { view: buffer_id });
			true
		} else {
			false
		}
	}

	fn save_jump(&mut self) {
		let buffer_id = self.focused_view();
		let cursor = self.buffer().cursor;
		self.buffer_mut().clear_undo_group();
		self.state
			.core
			.workspace
			.jump_list
			.push(crate::impls::JumpLocation { buffer_id, cursor });
	}
}

impl MacroAccess for Editor {
	fn record(&mut self) {
		self.state.core.workspace.macro_state.start_recording('q');
	}

	fn stop_recording(&mut self) {
		self.state.core.workspace.macro_state.stop_recording();
	}

	fn play(&mut self) {
		// TODO: Requires event loop integration
	}

	fn is_recording(&self) -> bool {
		self.state.core.workspace.macro_state.is_recording()
	}
}

impl CommandQueueAccess for Editor {
	fn queue_command(&mut self, name: &'static str, args: Vec<String>) {
		self.state.effects.queue_command(name, args);
	}
}

impl PaletteAccess for Editor {
	fn open_palette(&mut self) {
		self.state
			.effects
			.overlay_request(OverlayRequest::OpenModal {
				kind: "command_palette",
				args: vec![],
			});
	}

	fn close_palette(&mut self) {
		self.state
			.effects
			.overlay_request(OverlayRequest::CloseModal {
				reason: xeno_registry::actions::editor_ctx::OverlayCloseReason::Cancel,
			});
	}

	fn execute_palette(&mut self) {
		self.state
			.effects
			.overlay_request(OverlayRequest::CloseModal {
				reason: xeno_registry::actions::editor_ctx::OverlayCloseReason::Commit,
			});
	}

	fn palette_is_open(&self) -> bool {
		self.state.overlay_system.interaction.is_open()
	}
}

impl OptionAccess for Editor {
	fn option_raw(&self, key: OptionKey) -> OptionValue {
		self.resolve_option(self.focused_view(), key)
	}
}

impl EditorCapabilities for Editor {
	fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
		self
	}

	fn search(&mut self) -> Option<&mut dyn SearchAccess> {
		Some(self)
	}

	fn undo(&mut self) -> Option<&mut dyn UndoAccess> {
		Some(self)
	}

	fn edit(&mut self) -> Option<&mut dyn EditAccess> {
		Some(self)
	}

	fn motion(&mut self) -> Option<&mut dyn MotionAccess> {
		Some(self)
	}

	fn motion_dispatch(&mut self) -> Option<&mut dyn MotionDispatchAccess> {
		Some(self)
	}

	fn split_ops(&mut self) -> Option<&mut dyn SplitOps> {
		Some(self)
	}

	fn focus_ops(&mut self) -> Option<&mut dyn FocusOps> {
		Some(self)
	}

	fn viewport(&mut self) -> Option<&mut dyn ViewportAccess> {
		Some(self)
	}

	fn file_ops(&mut self) -> Option<&mut dyn FileOpsAccess> {
		Some(self)
	}

	fn jump_ops(&mut self) -> Option<&mut dyn JumpAccess> {
		Some(self)
	}

	fn macro_ops(&mut self) -> Option<&mut dyn MacroAccess> {
		Some(self)
	}

	fn command_queue(&mut self) -> Option<&mut dyn CommandQueueAccess> {
		Some(self)
	}

	fn palette(&mut self) -> Option<&mut dyn PaletteAccess> {
		Some(self)
	}

	fn option_ops(&self) -> Option<&dyn OptionAccess> {
		Some(self)
	}

	fn overlay(&mut self) -> Option<&mut dyn OverlayAccess> {
		Some(self)
	}

	fn open_search_prompt(&mut self, reverse: bool) {
		self.open_search(reverse);
	}

	fn is_readonly(&self) -> bool {
		self.buffer().is_readonly()
	}
}

#[cfg(test)]
mod tests;
