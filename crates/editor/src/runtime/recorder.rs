//! Session recording for replay-based integration testing.
//!
//! Serializes [`RuntimeEvent`]s to a line-oriented text format that
//! kitty-test-harness can parse and replay. Activated via the
//! `XENO_RECORD_SESSION` environment variable.
//!
//! # Format
//!
//! * Key events use the existing `Key` Display notation (`C-A-S-<code>`)
//! * Consecutive key lines form a batch (replayed as a single `send_text`)
//! * Blank lines separate batches (configurable pause on replay)
//! * Non-key events use prefixed lines: `mouse:`, `paste:`, `resize:`, `focus:`
//!
//! [`RuntimeEvent`]: super::RuntimeEvent

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use xeno_primitives::{Key, MouseEvent};

use super::RuntimeEvent;

/// Records runtime events to a text file for later replay.
pub(crate) struct EventRecorder {
	writer: BufWriter<File>,
	/// Tracks whether the previous event was a key, for batch grouping.
	last_was_key: bool,
}

impl EventRecorder {
	/// Opens a new recording file at `path`.
	pub(crate) fn new(path: &Path) -> std::io::Result<Self> {
		let file = File::create(path)?;
		Ok(Self {
			writer: BufWriter::new(file),
			last_was_key: false,
		})
	}

	/// Creates a recorder from the `XENO_RECORD_SESSION` env var, if set.
	pub(crate) fn from_env() -> Option<Self> {
		let path = std::env::var("XENO_RECORD_SESSION").ok()?;
		match Self::new(Path::new(&path)) {
			Ok(rec) => {
				tracing::info!("recording session to {path}");
				Some(rec)
			}
			Err(e) => {
				tracing::warn!("failed to open session recording at {path}: {e}");
				None
			}
		}
	}

	/// Records a single event. Errors are logged and swallowed.
	pub(crate) fn record(&mut self, ev: &RuntimeEvent) {
		if let Err(e) = self.record_inner(ev) {
			tracing::warn!("session recording write error: {e}");
		}
	}

	fn record_inner(&mut self, ev: &RuntimeEvent) -> std::io::Result<()> {
		match ev {
			RuntimeEvent::Key(key) => {
				self.last_was_key = true;
				writeln!(self.writer, "{key}")?;
			}
			RuntimeEvent::Mouse(mouse) => {
				self.flush_key_batch()?;
				write_mouse(&mut self.writer, mouse)?;
			}
			RuntimeEvent::Paste(content) => {
				self.flush_key_batch()?;
				use base64::Engine;
				let encoded = base64::engine::general_purpose::STANDARD.encode(content.as_bytes());
				writeln!(self.writer, "paste:{encoded}")?;
			}
			RuntimeEvent::WindowResized { cols, rows } => {
				self.flush_key_batch()?;
				writeln!(self.writer, "resize:{cols}x{rows}")?;
			}
			RuntimeEvent::FocusIn => {
				self.flush_key_batch()?;
				writeln!(self.writer, "focus:in")?;
			}
			RuntimeEvent::FocusOut => {
				self.flush_key_batch()?;
				writeln!(self.writer, "focus:out")?;
			}
		}
		self.writer.flush()
	}

	/// Inserts a blank line to mark a batch boundary when transitioning
	/// from key events to non-key events.
	fn flush_key_batch(&mut self) -> std::io::Result<()> {
		if self.last_was_key {
			writeln!(self.writer)?;
			self.last_was_key = false;
		}
		Ok(())
	}
}

fn write_mouse(w: &mut impl Write, mouse: &MouseEvent) -> std::io::Result<()> {
	match mouse {
		MouseEvent::Press { button, col, row, modifiers } => {
			write!(w, "mouse:press {} {col},{row}", button_str(*button))?;
			write_modifiers(w, modifiers)?;
			writeln!(w)?;
		}
		MouseEvent::Release { col, row } => {
			writeln!(w, "mouse:release {col},{row}")?;
		}
		MouseEvent::Drag { button, col, row, modifiers } => {
			write!(w, "mouse:drag {} {col},{row}", button_str(*button))?;
			write_modifiers(w, modifiers)?;
			writeln!(w)?;
		}
		MouseEvent::Scroll {
			direction,
			col,
			row,
			modifiers,
		} => {
			write!(w, "mouse:scroll {} {col},{row}", direction_str(*direction))?;
			write_modifiers(w, modifiers)?;
			writeln!(w)?;
		}
		MouseEvent::Move { col, row } => {
			writeln!(w, "mouse:move {col},{row}")?;
		}
	}
	Ok(())
}

fn button_str(b: xeno_primitives::MouseButton) -> &'static str {
	match b {
		xeno_primitives::MouseButton::Left => "left",
		xeno_primitives::MouseButton::Right => "right",
		xeno_primitives::MouseButton::Middle => "middle",
	}
}

fn direction_str(d: xeno_primitives::ScrollDirection) -> &'static str {
	match d {
		xeno_primitives::ScrollDirection::Up => "up",
		xeno_primitives::ScrollDirection::Down => "down",
		xeno_primitives::ScrollDirection::Left => "left",
		xeno_primitives::ScrollDirection::Right => "right",
	}
}

fn write_modifiers(w: &mut impl Write, m: &xeno_primitives::Modifiers) -> std::io::Result<()> {
	if m.ctrl || m.alt || m.shift {
		write!(w, " ")?;
		if m.ctrl {
			write!(w, "C-")?;
		}
		if m.alt {
			write!(w, "A-")?;
		}
		if m.shift {
			write!(w, "S-")?;
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use xeno_primitives::{Modifiers, MouseButton, ScrollDirection};

	use super::*;

	fn record_events(events: &[RuntimeEvent]) -> String {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("test.xsession");
		{
			let mut rec = EventRecorder::new(&path).unwrap();
			for ev in events {
				rec.record(ev);
			}
		}
		std::fs::read_to_string(&path).unwrap()
	}

	#[test]
	fn key_batch() {
		let output = record_events(&[
			RuntimeEvent::Key(Key::char('j')),
			RuntimeEvent::Key(Key::char('k')),
			RuntimeEvent::Key(Key::ctrl('x')),
		]);
		assert_eq!(output, "j\nk\nC-x\n");
	}

	#[test]
	fn non_key_flushes_batch() {
		let output = record_events(&[RuntimeEvent::Key(Key::char('j')), RuntimeEvent::FocusOut, RuntimeEvent::Key(Key::char('k'))]);
		assert_eq!(output, "j\n\nfocus:out\nk\n");
	}

	#[test]
	fn mouse_press() {
		let output = record_events(&[RuntimeEvent::Mouse(MouseEvent::Press {
			button: MouseButton::Left,
			col: 10,
			row: 5,
			modifiers: Modifiers::NONE,
		})]);
		assert_eq!(output, "mouse:press left 10,5\n");
	}

	#[test]
	fn mouse_scroll_with_modifiers() {
		let output = record_events(&[RuntimeEvent::Mouse(MouseEvent::Scroll {
			direction: ScrollDirection::Up,
			col: 10,
			row: 5,
			modifiers: Modifiers::CTRL,
		})]);
		assert_eq!(output, "mouse:scroll up 10,5 C-\n");
	}

	#[test]
	fn paste_base64() {
		let output = record_events(&[RuntimeEvent::Paste("hello world".to_string())]);
		assert_eq!(output, "paste:aGVsbG8gd29ybGQ=\n");
	}

	#[test]
	fn resize() {
		let output = record_events(&[RuntimeEvent::WindowResized { cols: 120, rows: 50 }]);
		assert_eq!(output, "resize:120x50\n");
	}

	#[test]
	fn focus() {
		let output = record_events(&[RuntimeEvent::FocusIn, RuntimeEvent::FocusOut]);
		assert_eq!(output, "focus:in\nfocus:out\n");
	}
}
