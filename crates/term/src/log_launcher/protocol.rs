//! Protocol types for log IPC between xeno and the log viewer.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// A log event sent from xeno to the log viewer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
	/// Timestamp when the event occurred.
	pub timestamp: SystemTime,
	/// Log level.
	pub level: Level,
	/// Target module path.
	pub target: String,
	/// The log message.
	pub message: String,
	/// Span context - stack of span names from outermost to innermost.
	pub spans: Vec<SpanInfo>,
	/// Key-value fields on the event.
	pub fields: Vec<(String, String)>,
}

/// Information about a span in the context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanInfo {
	/// Span name.
	pub name: String,
	/// Target/module of the span.
	pub target: String,
	/// Fields recorded on the span.
	pub fields: Vec<(String, String)>,
}

/// Log level matching tracing levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
	Trace,
	Debug,
	Info,
	Warn,
	Error,
}

impl Level {
	/// Returns a colored display string for the level.
	pub fn colored(&self) -> &'static str {
		match self {
			Level::Trace => "\x1b[35mTRACE\x1b[0m",
			Level::Debug => "\x1b[34mDEBUG\x1b[0m",
			Level::Info => "\x1b[32mINFO\x1b[0m",
			Level::Warn => "\x1b[33mWARN\x1b[0m",
			Level::Error => "\x1b[31mERROR\x1b[0m",
		}
	}
}

impl From<tracing::Level> for Level {
	fn from(level: tracing::Level) -> Self {
		match level {
			tracing::Level::TRACE => Level::Trace,
			tracing::Level::DEBUG => Level::Debug,
			tracing::Level::INFO => Level::Info,
			tracing::Level::WARN => Level::Warn,
			tracing::Level::ERROR => Level::Error,
		}
	}
}

impl From<&tracing::Level> for Level {
	fn from(level: &tracing::Level) -> Self {
		Self::from(*level)
	}
}

/// Span lifecycle events for tree rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanEvent {
	/// A new span was entered.
	Enter {
		id: u64,
		name: String,
		target: String,
		level: Level,
		fields: Vec<(String, String)>,
		parent_id: Option<u64>,
	},
	/// A span was exited.
	Exit { id: u64 },
	/// A span was closed (dropped).
	Close {
		id: u64,
		/// Duration the span was active, in microseconds.
		duration_us: u64,
	},
}

/// Wire message format - either a log event or span lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogMessage {
	Event(LogEvent),
	Span(SpanEvent),
}
