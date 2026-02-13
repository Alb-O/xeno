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
	use xeno_primitives::{Key, KeyCode};

	use super::*;

	async fn run_script(editor: &mut Editor, events: impl IntoIterator<Item = RuntimeEvent>) {
		for event in events {
			let _ = editor.on_event(event).await;
		}
	}

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

	#[tokio::test]
	async fn test_runtime_event_scripts_converge_for_inserted_text() {
		let esc = Key::new(KeyCode::Esc);

		let script_with_paste = vec![
			RuntimeEvent::WindowResized { cols: 80, rows: 24 },
			RuntimeEvent::Key(Key::char('i')),
			RuntimeEvent::Paste(String::from("abc")),
			RuntimeEvent::Key(esc),
		];

		let script_with_typed_keys = vec![
			RuntimeEvent::WindowResized { cols: 80, rows: 24 },
			RuntimeEvent::Key(Key::char('i')),
			RuntimeEvent::Key(Key::char('a')),
			RuntimeEvent::Key(Key::char('b')),
			RuntimeEvent::Key(Key::char('c')),
			RuntimeEvent::Key(esc),
		];

		let mut via_paste = Editor::new_scratch();
		let _ = via_paste.pump().await;
		run_script(&mut via_paste, script_with_paste).await;

		let mut via_keys = Editor::new_scratch();
		let _ = via_keys.pump().await;
		run_script(&mut via_keys, script_with_typed_keys).await;

		let text_via_paste = via_paste.buffer().with_doc(|doc| doc.content().to_string());
		let text_via_keys = via_keys.buffer().with_doc(|doc| doc.content().to_string());

		assert_eq!(text_via_paste, "abc");
		assert_eq!(text_via_paste, text_via_keys);
		assert_eq!(via_paste.mode(), via_keys.mode());
		assert_eq!(via_paste.statusline_render_plan(), via_keys.statusline_render_plan());
	}

	#[tokio::test]
	async fn test_runtime_event_scripts_converge_for_multiline_input() {
		let esc = Key::new(KeyCode::Esc);
		let enter = Key::new(KeyCode::Enter);

		let script_with_paste = vec![
			RuntimeEvent::WindowResized { cols: 80, rows: 24 },
			RuntimeEvent::Key(Key::char('i')),
			RuntimeEvent::Paste(String::from("a\r\nb")),
			RuntimeEvent::Key(esc),
		];

		let script_with_typed_keys = vec![
			RuntimeEvent::WindowResized { cols: 80, rows: 24 },
			RuntimeEvent::Key(Key::char('i')),
			RuntimeEvent::Key(Key::char('a')),
			RuntimeEvent::Key(enter),
			RuntimeEvent::Key(Key::char('b')),
			RuntimeEvent::Key(esc),
		];

		let mut via_paste = Editor::new_scratch();
		let _ = via_paste.pump().await;
		run_script(&mut via_paste, script_with_paste).await;

		let mut via_keys = Editor::new_scratch();
		let _ = via_keys.pump().await;
		run_script(&mut via_keys, script_with_typed_keys).await;

		let text_via_paste = via_paste.buffer().with_doc(|doc| doc.content().to_string());
		let text_via_keys = via_keys.buffer().with_doc(|doc| doc.content().to_string());

		assert_eq!(text_via_paste, "a\nb");
		assert_eq!(text_via_paste, text_via_keys);
		assert_eq!(via_paste.mode(), via_keys.mode());
		assert_eq!(via_paste.statusline_render_plan(), via_keys.statusline_render_plan());
	}

	#[tokio::test]
	async fn test_runtime_event_scripts_converge_for_command_palette_completion() {
		let mut via_paste = Editor::new_scratch();
		via_paste.handle_window_resize(120, 30);
		assert!(via_paste.open_command_palette());
		let _ = via_paste.pump().await;
		run_script(&mut via_paste, vec![RuntimeEvent::Paste(String::from("set"))]).await;

		let mut via_keys = Editor::new_scratch();
		via_keys.handle_window_resize(120, 30);
		assert!(via_keys.open_command_palette());
		let _ = via_keys.pump().await;
		run_script(
			&mut via_keys,
			vec![
				RuntimeEvent::Key(Key::char('s')),
				RuntimeEvent::Key(Key::char('e')),
				RuntimeEvent::Key(Key::char('t')),
			],
		)
		.await;

		assert_eq!(via_paste.overlay_kind(), via_keys.overlay_kind());
		assert_eq!(via_paste.completion_popup_render_plan(), via_keys.completion_popup_render_plan());
		assert_eq!(via_paste.statusline_render_plan(), via_keys.statusline_render_plan());
	}
}
