//! Protocol types for log IPC between xeno and the log viewer.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Xeno layer categories for log viewer filtering.
///
/// Maps tracing targets to logical subsystems for interactive filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum XenoLayer {
	Core,
	Api,
	Lsp,
	Lang,
	Config,
	Ui,
	Registry,
	#[default]
	External,
}

const LAYER_INFO: &[(XenoLayer, &str, &str, &[&str])] = &[
	(
		XenoLayer::Core,
		"CORE",
		"\x1b[95mCORE\x1b[0m",
		&["xeno_primitives", "xeno_registry"],
	),
	(
		XenoLayer::Api,
		"API",
		"\x1b[96mAPI \x1b[0m",
		&["xeno_editor"],
	),
	(XenoLayer::Lsp, "LSP", "\x1b[93mLSP \x1b[0m", &["xeno_lsp"]),
	(
		XenoLayer::Lang,
		"LANG",
		"\x1b[94mLANG\x1b[0m",
		&["xeno_language"],
	),
	(
		XenoLayer::Config,
		"CFG",
		"\x1b[90mCFG \x1b[0m",
		&["xeno_config"],
	),
	(
		XenoLayer::Ui,
		"UI",
		"\x1b[92mUI  \x1b[0m",
		&["xeno_term", "xeno_tui"],
	),
	(
		XenoLayer::Registry,
		"REG",
		"\x1b[90mREG \x1b[0m",
		&["xeno_registry"],
	),
	(XenoLayer::External, "EXT", "\x1b[90mEXT \x1b[0m", &[]),
];

impl XenoLayer {
	pub fn short_name(&self) -> &'static str {
		LAYER_INFO
			.iter()
			.find(|(l, ..)| l == self)
			.map(|(_, n, ..)| *n)
			.unwrap_or("EXT")
	}

	pub fn colored(&self) -> &'static str {
		LAYER_INFO
			.iter()
			.find(|(l, ..)| l == self)
			.map(|(_, _, c, _)| *c)
			.unwrap_or("\x1b[90mEXT \x1b[0m")
	}

	pub fn from_target(target: &str) -> XenoLayer {
		for (layer, _, _, prefixes) in LAYER_INFO {
			if prefixes.iter().any(|p| target.starts_with(p)) {
				return *layer;
			}
		}
		XenoLayer::External
	}
}

/// A log event sent from xeno to the log viewer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
	/// Timestamp when the event occurred.
	pub timestamp: SystemTime,
	/// Log level.
	pub level: Level,
	/// Xeno layer category (derived from target).
	pub layer: XenoLayer,
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
	/// Returns a block-style (background colored) display string.
	pub fn colored(&self) -> &'static str {
		match self {
			Level::Trace => "\x1b[100;97m TRACE \x1b[0m",
			Level::Debug => "\x1b[44;1;97m DEBUG \x1b[0m",
			Level::Info => "\x1b[102;30m INFO \x1b[0m",
			Level::Warn => "\x1b[103;30m WARN \x1b[0m",
			Level::Error => "\x1b[41;1;97m ERROR \x1b[0m",
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
		layer: XenoLayer,
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

/// Wire message format, either a log event or span lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogMessage {
	Event(LogEvent),
	Span(SpanEvent),
	Disconnected,
}
