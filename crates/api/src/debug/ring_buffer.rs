//! Thread-safe ring buffer for log entries.

use std::collections::VecDeque;
use std::sync::{LazyLock, RwLock};

/// Maximum number of log entries to retain.
pub const MAX_LOG_ENTRIES: usize = 1000;

/// Global log buffer instance.
pub static LOG_BUFFER: LazyLock<LogRingBuffer> = LazyLock::new(LogRingBuffer::new);

/// Log severity levels, ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
	/// Verbose diagnostic information for debugging.
	Trace,
	/// Debugging information for developers.
	Debug,
	/// General informational messages.
	Info,
	/// Warnings about potential issues.
	Warn,
	/// Error conditions that should be addressed.
	Error,
}

impl From<tracing::Level> for LogLevel {
	fn from(level: tracing::Level) -> Self {
		match level {
			tracing::Level::ERROR => LogLevel::Error,
			tracing::Level::WARN => LogLevel::Warn,
			tracing::Level::INFO => LogLevel::Info,
			tracing::Level::DEBUG => LogLevel::Debug,
			tracing::Level::TRACE => LogLevel::Trace,
		}
	}
}

/// Context from the enclosing action span, if any.
#[derive(Debug, Clone, Default)]
pub struct ActionSpanContext {
	/// Action name (e.g., "move_line_down").
	pub action_name: Option<String>,
	/// Full action ID (e.g., "evildoer-stdlib::move_line_down").
	pub action_id: Option<String>,
	/// Repeat count for the action.
	pub count: Option<usize>,
	/// Whether selection is being extended.
	pub extend: Option<bool>,
	/// Character argument for pending actions (e.g., 'f', 'r').
	pub char_arg: Option<char>,
}

/// A single log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
	/// Severity level of this log entry.
	pub level: LogLevel,
	/// Module or target that produced this log.
	pub target: String,
	/// The log message content.
	pub message: String,
	/// Context from enclosing action span, if event occurred within one.
	pub action_ctx: Option<ActionSpanContext>,
}

/// Thread-safe ring buffer for log entries.
pub struct LogRingBuffer {
	/// The underlying deque storing log entries, protected by a read-write lock.
	entries: RwLock<VecDeque<LogEntry>>,
}

impl LogRingBuffer {
	/// Creates a new empty log ring buffer.
	pub fn new() -> Self {
		Self {
			entries: RwLock::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)),
		}
	}

	/// Pushes a new log entry, evicting the oldest if at capacity.
	pub fn push(&self, entry: LogEntry) {
		let mut entries = self.entries.write().unwrap();
		if entries.len() >= MAX_LOG_ENTRIES {
			entries.pop_front();
		}
		entries.push_back(entry);
	}

	/// Returns a snapshot of all log entries.
	pub fn entries(&self) -> Vec<LogEntry> {
		self.entries.read().unwrap().iter().cloned().collect()
	}

	/// Returns the current number of log entries.
	pub fn len(&self) -> usize {
		self.entries.read().unwrap().len()
	}

	/// Returns true if the buffer contains no entries.
	pub fn is_empty(&self) -> bool {
		self.entries.read().unwrap().is_empty()
	}

	/// Removes all log entries from the buffer.
	pub fn clear(&self) {
		self.entries.write().unwrap().clear();
	}
}

impl Default for LogRingBuffer {
	fn default() -> Self {
		Self::new()
	}
}
