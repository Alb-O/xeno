use std::time::Duration;

use xeno_primitives::{Key, Mode, MouseEvent};

use crate::Editor;

#[derive(Debug, Clone, Copy)]
pub struct LoopDirective {
	pub poll_timeout: Option<Duration>,
	pub needs_redraw: bool,
	pub cursor_style: CursorStyle,
	pub should_quit: bool,
}

/// Editor-defined cursor style (term maps to termina CSI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
	#[default]
	Block,
	Beam,
	Underline,
	Hidden,
}

/// Frontend-agnostic event stream consumed by the editor runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
	Key(Key),
	Mouse(MouseEvent),
	Paste(String),
	/// Viewport size expressed in text-grid cells.
	WindowResized {
		cols: u16,
		rows: u16,
	},
	FocusIn,
	FocusOut,
}

/// Runtime policy constants.
const HOOK_BUDGET_FAST: crate::hook_runtime::HookDrainBudget = crate::hook_runtime::HookDrainBudget {
	duration: Duration::from_millis(1),
	max_completions: 32,
};
const HOOK_BUDGET_SLOW: crate::hook_runtime::HookDrainBudget = crate::hook_runtime::HookDrainBudget {
	duration: Duration::from_millis(3),
	max_completions: 64,
};

impl Editor {
	/// Runs one maintenance cycle.
	pub async fn pump(&mut self) -> LoopDirective {
		self.ui_tick();
		self.tick();

		let fs_changed = self.state.filesystem.pump(crate::filesystem::PumpBudget {
			max_index_msgs: 32,
			max_search_msgs: 8,
			max_time: Duration::from_millis(4),
		});
		if fs_changed {
			self.interaction_refresh_file_picker();
			self.frame_mut().needs_redraw = true;
		}

		let hook_budget = if matches!(self.mode(), Mode::Insert) {
			HOOK_BUDGET_FAST
		} else {
			HOOK_BUDGET_SLOW
		};

		let hook_stats = self.hook_runtime_mut().drain_budget(hook_budget).await;
		self.metrics().record_hook_tick(hook_stats.completed, hook_stats.pending);

		let should_quit = self.drain_command_queue().await || self.take_quit_request();

		if self.state.frame.pending_overlay_commit {
			self.state.frame.pending_overlay_commit = false;
			self.interaction_commit().await;
		}

		let msg_dirty = self.drain_messages();
		if msg_dirty.needs_redraw() {
			self.frame_mut().needs_redraw = true;
		}

		#[cfg(feature = "lsp")]
		if !self.state.frame.pending_workspace_edits.is_empty() {
			let edits = std::mem::take(&mut self.state.frame.pending_workspace_edits);
			for edit in edits {
				if let Err(err) = self.apply_workspace_edit(edit).await {
					self.notify(xeno_registry::notifications::keys::error(err.to_string()));
				}
			}
			self.frame_mut().needs_redraw = true;
		}

		let needs_redraw = self.frame().needs_redraw;

		let poll_timeout = if matches!(self.mode(), Mode::Insert) || self.any_panel_open() || needs_redraw {
			Some(Duration::from_millis(16))
		} else {
			Some(Duration::from_millis(50))
		};

		LoopDirective {
			poll_timeout,
			needs_redraw,
			cursor_style: self.derive_cursor_style(),
			should_quit,
		}
	}

	/// Handle a single frontend event and then run `pump`.
	pub async fn on_event(&mut self, ev: RuntimeEvent) -> LoopDirective {
		match ev {
			RuntimeEvent::Key(key) => {
				let _ = self.handle_key(key).await;
			}
			RuntimeEvent::Mouse(mouse) => {
				let _ = self.handle_mouse(mouse).await;
			}
			RuntimeEvent::Paste(content) => {
				self.handle_paste(content);
			}
			RuntimeEvent::WindowResized { cols, rows } => {
				self.handle_window_resize(cols, rows);
			}
			RuntimeEvent::FocusIn => {
				self.handle_focus_in();
			}
			RuntimeEvent::FocusOut => {
				self.handle_focus_out();
			}
		}

		self.pump().await
	}

	fn derive_cursor_style(&self) -> CursorStyle {
		let style = self.ui().cursor_style().unwrap_or_else(|| match self.mode() {
			Mode::Insert => CursorStyle::Beam,
			_ => CursorStyle::Block,
		});
		style
	}
}

#[cfg(test)]
mod tests {
	use xeno_primitives::Key;

	use super::*;

	#[tokio::test]
	async fn test_on_event_implies_pump() {
		let mut editor = Editor::new_scratch();

		// Initial pump to clear startup state
		let _ = editor.pump().await;

		// Any event should trigger maintenance
		let ev = RuntimeEvent::Key(Key::char('i'));

		let dir = editor.on_event(ev).await;

		// Insert mode should set fast timeout
		assert_eq!(dir.poll_timeout, Some(Duration::from_millis(16)));
		assert_eq!(editor.mode(), Mode::Insert);
	}
}
