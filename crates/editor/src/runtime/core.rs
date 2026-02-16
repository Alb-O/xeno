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

impl Editor {
	/// Runs one maintenance cycle.
	pub async fn pump(&mut self) -> LoopDirective {
		super::pump::run_pump_cycle(self).await
	}

	/// Handle a single frontend event and then run `pump`.
	pub async fn on_event(&mut self, ev: RuntimeEvent) -> LoopDirective {
		if let Some(rec) = &mut self.state.recorder {
			rec.record(&ev);
		}
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

	pub(crate) fn derive_cursor_style(&self) -> CursorStyle {
		self.ui().cursor_style().unwrap_or_else(|| match self.mode() {
			Mode::Insert => CursorStyle::Beam,
			_ => CursorStyle::Block,
		})
	}

	#[cfg(test)]
	pub(crate) async fn pump_with_report(&mut self) -> (LoopDirective, super::pump::PumpCycleReport) {
		super::pump::run_pump_cycle_with_report(self).await
	}
}
