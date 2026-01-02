//! Debug panel for displaying tracing logs.
//!
//! Provides [`DebugPanel`], which implements [`SplitBuffer`] to display live
//! tracing output in a split view. Logs are captured via a global ring buffer
//! that a tracing subscriber writes to.
//!
//! # Usage
//!
//! Add [`DebugPanelLayer`] to your tracing subscriber:
//!
//! ```ignore
//! use tracing_subscriber::prelude::*;
//! use evildoer_api::debug::DebugPanelLayer;
//!
//! tracing_subscriber::registry()
//!     .with(DebugPanelLayer::new())
//!     .init();
//! ```

mod ring_buffer;
mod selection;
mod tracing_layer;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use evildoer_registry::panels::{
	SplitAttrs, SplitBuffer, SplitCell, SplitColor, SplitCursor, SplitCursorStyle,
	SplitEventResult, SplitKey, SplitKeyCode, SplitModifiers, SplitMouse, SplitMouseAction,
	SplitMouseButton, SplitSize,
};
pub use ring_buffer::{ActionSpanContext, LOG_BUFFER, LogEntry, LogLevel, MAX_LOG_ENTRIES};
pub use tracing_layer::DebugPanelLayer;

use self::selection::DebugSelection;

/// Counter for generating unique debug panel instance IDs.
static NEXT_DEBUG_PANEL_ID: AtomicU64 = AtomicU64::new(0);

/// A debug panel that displays tracing logs.
///
/// Reads from a global log ring buffer populated by a tracing subscriber.
/// Supports scrolling, filtering by log level, and auto-scroll to follow new logs.
pub struct DebugPanel {
	/// Current panel dimensions.
	size: SplitSize,
	/// Number of lines scrolled from the bottom (0 = at bottom).
	scroll_offset: usize,
	/// Whether to automatically scroll to show new entries.
	auto_scroll: bool,
	/// Minimum log level to display.
	min_level: LogLevel,
	/// Cached log count for detecting new entries.
	last_log_count: usize,
	/// Current text selection, if any.
	selection: Option<DebugSelection>,
}

impl Default for DebugPanel {
	fn default() -> Self {
		Self::new()
	}
}

impl DebugPanel {
	/// Creates a new debug panel with default settings.
	pub fn new() -> Self {
		Self {
			size: SplitSize::new(80, 24),
			scroll_offset: 0,
			auto_scroll: true,
			min_level: LogLevel::Trace,
			last_log_count: 0,
			selection: None,
		}
	}

	/// Returns a unique ID for a new debug panel instance.
	pub fn next_id() -> u64 {
		NEXT_DEBUG_PANEL_ID.fetch_add(1, Ordering::Relaxed)
	}

	/// Returns the log entries currently visible in the viewport.
	fn visible_entries(&self) -> Vec<LogEntry> {
		let filtered: Vec<_> = LOG_BUFFER
			.entries()
			.into_iter()
			.filter(|e| e.level >= self.min_level)
			.collect();

		let height = self.size.height as usize;
		if filtered.is_empty() || height == 0 {
			return Vec::new();
		}

		let total = filtered.len();
		let end = total.saturating_sub(self.scroll_offset);
		let start = end.saturating_sub(height);

		filtered[start..end].to_vec()
	}

	/// Returns the count of log entries matching the current filter.
	fn filtered_count(&self) -> usize {
		LOG_BUFFER
			.entries()
			.into_iter()
			.filter(|e| e.level >= self.min_level)
			.count()
	}

	/// Scrolls up by the given number of lines.
	fn scroll_up(&mut self, lines: usize) {
		let max_offset = self
			.filtered_count()
			.saturating_sub(self.size.height as usize);
		self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
		self.auto_scroll = false;
	}

	/// Scrolls down by the given number of lines.
	fn scroll_down(&mut self, lines: usize) {
		if self.scroll_offset <= lines {
			self.scroll_offset = 0;
			self.auto_scroll = true;
		} else {
			self.scroll_offset -= lines;
		}
	}

	/// Cycles through log level filters (Trace -> Debug -> Info -> Warn -> Error).
	fn cycle_level_filter(&mut self) {
		self.min_level = match self.min_level {
			LogLevel::Trace => LogLevel::Debug,
			LogLevel::Debug => LogLevel::Info,
			LogLevel::Info => LogLevel::Warn,
			LogLevel::Warn => LogLevel::Error,
			LogLevel::Error => LogLevel::Trace,
		};
	}
}

impl SplitBuffer for DebugPanel {
	fn id(&self) -> &str {
		"debug"
	}

	fn resize(&mut self, size: SplitSize) {
		self.size = size;
	}

	fn tick(&mut self, _delta: Duration) -> SplitEventResult {
		let current_count = LOG_BUFFER.len();
		if current_count != self.last_log_count {
			self.last_log_count = current_count;
			if self.auto_scroll {
				self.scroll_offset = 0;
			}
			SplitEventResult {
				needs_redraw: true,
				..Default::default()
			}
		} else {
			SplitEventResult::ignored()
		}
	}

	fn handle_key(&mut self, key: SplitKey) -> SplitEventResult {
		match key.code {
			SplitKeyCode::Escape => return SplitEventResult::consumed().with_release_focus(),
			SplitKeyCode::Up | SplitKeyCode::Char('k') => self.scroll_up(1),
			SplitKeyCode::Down | SplitKeyCode::Char('j') => self.scroll_down(1),
			SplitKeyCode::PageUp => self.scroll_up(self.size.height as usize / 2),
			SplitKeyCode::PageDown => self.scroll_down(self.size.height as usize / 2),
			SplitKeyCode::Char('g') if key.modifiers == SplitModifiers::NONE => {
				self.scroll_offset = self
					.filtered_count()
					.saturating_sub(self.size.height as usize);
				self.auto_scroll = false;
			}
			SplitKeyCode::Char('G') | SplitKeyCode::End => {
				self.scroll_offset = 0;
				self.auto_scroll = true;
			}
			SplitKeyCode::Char('f') => self.cycle_level_filter(),
			SplitKeyCode::Char('c') => {
				LOG_BUFFER.clear();
				self.scroll_offset = 0;
			}
			_ => return SplitEventResult::ignored(),
		}
		SplitEventResult::consumed()
	}

	fn handle_mouse(&mut self, mouse: SplitMouse) -> SplitEventResult {
		let (row, col) = (mouse.position.y, mouse.position.x);

		match mouse.action {
			SplitMouseAction::ScrollUp => {
				self.scroll_up(3);
				SplitEventResult::consumed()
			}
			SplitMouseAction::ScrollDown => {
				self.scroll_down(3);
				SplitEventResult::consumed()
			}
			SplitMouseAction::Press(SplitMouseButton::Left) => {
				self.selection = Some(DebugSelection {
					anchor_row: row,
					anchor_col: col,
					cursor_row: row,
					cursor_col: col,
				});
				SplitEventResult::consumed()
			}
			SplitMouseAction::Drag(SplitMouseButton::Left) => {
				if let Some(sel) = &mut self.selection {
					sel.cursor_row = row;
					sel.cursor_col = col;
				}
				SplitEventResult::consumed()
			}
			SplitMouseAction::Release(SplitMouseButton::Left) => SplitEventResult::consumed(),
			SplitMouseAction::Press(SplitMouseButton::Right | SplitMouseButton::Middle) => {
				self.selection = None;
				SplitEventResult::consumed()
			}
			_ => SplitEventResult::ignored(),
		}
	}

	fn is_selected(&self, row: u16, col: u16) -> bool {
		self.selection.is_some_and(|sel| sel.contains(row, col))
	}

	fn size(&self) -> SplitSize {
		self.size
	}

	fn cursor(&self) -> Option<SplitCursor> {
		Some(SplitCursor {
			row: 0,
			col: 0,
			style: SplitCursorStyle::Hidden,
		})
	}

	fn for_each_cell(&self, f: &mut dyn FnMut(u16, u16, &SplitCell)) {
		let entries = self.visible_entries();
		let width = self.size.width as usize;

		for (row, entry) in entries.iter().enumerate() {
			let row = row as u16;

			let (level_color, level_str) = match entry.level {
				LogLevel::Error => (SplitColor::Indexed(1), "ERR"),
				LogLevel::Warn => (SplitColor::Indexed(3), "WRN"),
				LogLevel::Info => (SplitColor::Indexed(2), "INF"),
				LogLevel::Debug => (SplitColor::Indexed(4), "DBG"),
				LogLevel::Trace => (SplitColor::Indexed(8), "TRC"),
			};

			let mut col: u16 = 0;

			for ch in level_str.chars() {
				if (col as usize) >= width {
					break;
				}
				f(
					row,
					col,
					&SplitCell::new(ch.to_string())
						.with_fg(level_color)
						.with_attrs(SplitAttrs::BOLD),
				);
				col += 1;
			}

			if (col as usize) < width {
				f(row, col, &SplitCell::new(" "));
				col += 1;
			}

			if let Some(ref action_ctx) = entry.action_ctx
				&& let Some(ref action_name) = action_ctx.action_name
			{
				let action_color = SplitColor::Indexed(5);

				if (col as usize) < width {
					f(row, col, &SplitCell::new("[").with_fg(action_color));
					col += 1;
				}

				for ch in action_name.chars() {
					if (col as usize) >= width {
						break;
					}
					f(
						row,
						col,
						&SplitCell::new(ch.to_string())
							.with_fg(action_color)
							.with_attrs(SplitAttrs::BOLD),
					);
					col += 1;
				}

				// Show char_arg if present (e.g., for find char 'f')
				if let Some(ch) = action_ctx.char_arg {
					if (col as usize) < width {
						f(row, col, &SplitCell::new("'").with_fg(action_color));
						col += 1;
					}
					if (col as usize) < width {
						f(
							row,
							col,
							&SplitCell::new(ch.to_string()).with_fg(action_color),
						);
						col += 1;
					}
					if (col as usize) < width {
						f(row, col, &SplitCell::new("'").with_fg(action_color));
						col += 1;
					}
				}

				if let Some(count) = action_ctx.count
					&& count > 1
				{
					let count_str = format!("x{}", count);
					for ch in count_str.chars() {
						if (col as usize) >= width {
							break;
						}
						f(
							row,
							col,
							&SplitCell::new(ch.to_string()).with_fg(action_color),
						);
						col += 1;
					}
				}

				if (col as usize) < width {
					f(row, col, &SplitCell::new("]").with_fg(action_color));
					col += 1;
				}

				if (col as usize) < width {
					f(row, col, &SplitCell::new(" "));
					col += 1;
				}
			}

			let target_color = SplitColor::Indexed(6);
			for ch in entry.target.chars() {
				if (col as usize) >= width {
					break;
				}
				f(
					row,
					col,
					&SplitCell::new(ch.to_string()).with_fg(target_color),
				);
				col += 1;
			}

			if (col as usize) < width {
				f(row, col, &SplitCell::new(":"));
				col += 1;
			}
			if (col as usize) < width {
				f(row, col, &SplitCell::new(" "));
				col += 1;
			}

			for ch in entry.message.chars() {
				if (col as usize) >= width {
					break;
				}
				if ch == '\n' || ch == '\r' {
					continue;
				}
				f(row, col, &SplitCell::new(ch.to_string()));
				col += 1;
			}
		}

		if entries.is_empty() {
			let msg = format!(
				"No logs (filter: {:?}) - Press 'f' to cycle filter",
				self.min_level
			);
			for (col, ch) in msg.chars().enumerate() {
				if col >= width {
					break;
				}
				f(
					0,
					col as u16,
					&SplitCell::new(ch.to_string()).with_fg(SplitColor::Indexed(8)),
				);
			}
		}
	}
}
