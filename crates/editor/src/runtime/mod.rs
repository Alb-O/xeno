use std::time::Duration;

use xeno_primitives::Mode;

use crate::impls::Editor;

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

/// Runtime policy constants.
const HOOK_BUDGET_FAST: crate::hook_runtime::HookDrainBudget =
	crate::hook_runtime::HookDrainBudget {
		duration: Duration::from_millis(1),
		max_completions: 32,
	};
const HOOK_BUDGET_SLOW: crate::hook_runtime::HookDrainBudget =
	crate::hook_runtime::HookDrainBudget {
		duration: Duration::from_millis(3),
		max_completions: 64,
	};

impl Editor {
	/// Runs one maintenance cycle.
	pub async fn pump(&mut self) -> LoopDirective {
		self.ui_tick();
		self.tick();

		let hook_budget = if matches!(self.mode(), Mode::Insert) {
			HOOK_BUDGET_FAST
		} else {
			HOOK_BUDGET_SLOW
		};

		let hook_stats = self.hook_runtime_mut().drain_budget(hook_budget).await;
		self.metrics()
			.record_hook_tick(hook_stats.completed, hook_stats.pending);

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

		let poll_timeout =
			if matches!(self.mode(), Mode::Insert) || self.any_panel_open() || needs_redraw {
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

	/// Handle a single terminal event and then run `pump`.
	pub async fn on_event(&mut self, ev: termina::event::Event) -> LoopDirective {
		match ev {
			termina::event::Event::Key(key)
				if matches!(
					key.kind,
					termina::event::KeyEventKind::Press | termina::event::KeyEventKind::Repeat
				) =>
			{
				let _ = self.handle_key(key).await;
			}
			termina::event::Event::Mouse(mouse) => {
				let _ = self.handle_mouse(mouse).await;
			}
			termina::event::Event::Paste(content) => {
				self.handle_paste(content);
			}
			termina::event::Event::WindowResized(size) => {
				self.handle_window_resize(size.cols, size.rows);
			}
			termina::event::Event::FocusIn => {
				self.handle_focus_in();
			}
			termina::event::Event::FocusOut => {
				self.handle_focus_out();
			}
			_ => {}
		}

		self.pump().await
	}

	fn derive_cursor_style(&self) -> CursorStyle {
		use termina::style::CursorStyle as TerminaStyle;

		let style = self
			.ui()
			.cursor_style()
			.unwrap_or_else(|| match self.mode() {
				Mode::Insert => TerminaStyle::SteadyBar,
				_ => TerminaStyle::SteadyBlock,
			});

		match style {
			TerminaStyle::SteadyBar | TerminaStyle::BlinkingBar => CursorStyle::Beam,
			TerminaStyle::SteadyUnderline | TerminaStyle::BlinkingUnderline => {
				CursorStyle::Underline
			}
			_ => CursorStyle::Block,
		}
	}
}

#[cfg(test)]
mod tests {
	use termina::event::{Event, KeyCode, KeyEvent, KeyEventKind, Modifiers};

	use super::*;

	#[tokio::test]
	async fn test_on_event_implies_pump() {
		let mut editor = Editor::new_scratch();

		// Initial pump to clear startup state
		let _ = editor.pump().await;

		// Any event should trigger maintenance
		let ev = Event::Key(KeyEvent {
			code: KeyCode::Char('i'),
			modifiers: Modifiers::NONE,
			kind: KeyEventKind::Press,
			state: termina::event::KeyEventState::NONE,
		});

		let dir = editor.on_event(ev).await;

		// Insert mode should set fast timeout
		assert_eq!(dir.poll_timeout, Some(Duration::from_millis(16)));
		assert_eq!(editor.mode(), Mode::Insert);
	}

	#[tokio::test]
	async fn test_needs_redraw_cleared_by_render() {
		let mut editor = Editor::new_scratch();
		editor.frame_mut().needs_redraw = true;

		let backend = xeno_tui::backend::TestBackend::new(80, 24);
		let mut terminal = xeno_tui::Terminal::new(backend).unwrap();

		terminal
			.draw(|frame| {
				editor.render(frame);
			})
			.unwrap();

		assert!(!editor.frame().needs_redraw);
	}
}
