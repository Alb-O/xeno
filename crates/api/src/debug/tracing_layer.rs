//! Tracing layer that writes log events to the debug panel's ring buffer.
//!
//! This layer captures tracing events and enriches them with context from
//! enclosing spans. When an event occurs within an "action" span, the action's
//! metadata (name, id, count, extend) is attached to the log entry.

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::ring_buffer::{ActionSpanContext, LOG_BUFFER, LogEntry, LogLevel};

/// Data stored per-span for action context extraction.
#[derive(Debug, Default)]
pub struct ActionSpanData {
	/// Name of the action (e.g., "move_left", "insert_char").
	pub action_name: Option<String>,
	/// Unique identifier for the action definition.
	pub action_id: Option<String>,
	/// Repeat count for the action.
	pub count: Option<usize>,
	/// Whether the action extends the current selection.
	pub extend: Option<bool>,
	/// Optional character argument for the action.
	pub char_arg: Option<char>,
}

/// A [`tracing_subscriber::Layer`] that writes events to [`LOG_BUFFER`] for the debug panel.
///
/// Tracks "action" spans and attaches their context (name, id, count, extend)
/// to nested events, enabling correlation between log entries and the actions
/// that triggered them.
pub struct DebugPanelLayer;

impl DebugPanelLayer {
	/// Creates a new debug panel tracing layer.
	pub fn new() -> Self {
		Self
	}
}

impl Default for DebugPanelLayer {
	fn default() -> Self {
		Self::new()
	}
}

/// Visitor for extracting the message field from events.
struct MessageVisitor {
	/// The extracted message string.
	message: String,
	/// Additional key-value fields from the event.
	fields: Vec<(String, String)>,
}

impl MessageVisitor {
	/// Creates a new message visitor with empty fields.
	fn new() -> Self {
		Self {
			message: String::new(),
			fields: Vec::new(),
		}
	}
}

impl Visit for MessageVisitor {
	fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
		let formatted = format!("{:?}", value);
		// Debug formatting adds quotes around strings - strip them for cleaner output
		let cleaned = formatted
			.strip_prefix('"')
			.and_then(|s| s.strip_suffix('"'))
			.map(|s| {
				// Also unescape any escaped characters common in JSON
				s.replace("\\\"", "\"")
					.replace("\\\\", "\\")
					.replace("\\n", "\n")
					.replace("\\t", "\t")
			})
			.unwrap_or(formatted);

		if field.name() == "message" {
			self.message = cleaned;
		} else {
			self.fields.push((field.name().to_string(), cleaned));
		}
	}

	fn record_str(&mut self, field: &Field, value: &str) {
		if field.name() == "message" {
			self.message = value.to_string();
		} else {
			self.fields
				.push((field.name().to_string(), value.to_string()));
		}
	}

	fn record_i64(&mut self, field: &Field, value: i64) {
		self.fields
			.push((field.name().to_string(), value.to_string()));
	}

	fn record_u64(&mut self, field: &Field, value: u64) {
		self.fields
			.push((field.name().to_string(), value.to_string()));
	}

	fn record_bool(&mut self, field: &Field, value: bool) {
		self.fields
			.push((field.name().to_string(), value.to_string()));
	}
}

/// Visitor for extracting action span fields.
struct ActionSpanVisitor(ActionSpanData);

impl Visit for ActionSpanVisitor {
	fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
		match field.name() {
			"name" => {
				self.0.action_name = Some(format!("{:?}", value).trim_matches('"').to_string())
			}
			"id" => self.0.action_id = Some(format!("{:?}", value).trim_matches('"').to_string()),
			_ => {}
		}
	}

	fn record_str(&mut self, field: &Field, value: &str) {
		match field.name() {
			"name" => self.0.action_name = Some(value.to_string()),
			"id" => self.0.action_id = Some(value.to_string()),
			"char_arg" => {
				if let Some(ch) = value.chars().next() {
					self.0.char_arg = Some(ch);
				}
			}
			_ => {}
		}
	}

	fn record_u64(&mut self, field: &Field, value: u64) {
		if field.name() == "count" {
			self.0.count = Some(value as usize);
		}
	}

	fn record_bool(&mut self, field: &Field, value: bool) {
		if field.name() == "extend" {
			self.0.extend = Some(value);
		}
	}

	fn record_i64(&mut self, _field: &Field, _value: i64) {}
}

impl<S> tracing_subscriber::Layer<S> for DebugPanelLayer
where
	S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
	fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
		if attrs.metadata().name() != "action" {
			return;
		}
		let Some(span) = ctx.span(id) else {
			return;
		};
		let mut visitor = ActionSpanVisitor(ActionSpanData::default());
		attrs.record(&mut visitor);
		span.extensions_mut().insert(visitor.0);
	}

	fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
		// Filter out noisy external crate logs
		let target = event.metadata().target();
		if !target.starts_with("evildoer") {
			// Only allow WARN+ from external crates
			if *event.metadata().level() > Level::WARN {
				return;
			}
		}

		let level = match *event.metadata().level() {
			Level::ERROR => LogLevel::Error,
			Level::WARN => LogLevel::Warn,
			Level::INFO => LogLevel::Info,
			Level::DEBUG => LogLevel::Debug,
			Level::TRACE => LogLevel::Trace,
		};

		let mut visitor = MessageVisitor::new();
		event.record(&mut visitor);

		let mut message = if visitor.message.is_empty() {
			event.metadata().name().to_string()
		} else {
			visitor.message
		};

		if !visitor.fields.is_empty() {
			let fields_str = visitor
				.fields
				.iter()
				.map(|(k, v)| format!("{}={}", k, v))
				.collect::<Vec<_>>()
				.join(" ");
			message = if message.is_empty() {
				fields_str
			} else {
				format!("{} {{{}}}", message, fields_str)
			};
		}

		// Use event_scope to get spans associated with this event, which handles
		// explicit parent spans better than lookup_current()
		let action_ctx = ctx.event_scope(event).and_then(|scope| {
			for span in scope {
				if let Some(data) = span.extensions().get::<ActionSpanData>() {
					return Some(ActionSpanContext {
						action_name: data.action_name.clone(),
						action_id: data.action_id.clone(),
						count: data.count,
						extend: data.extend,
						char_arg: data.char_arg,
					});
				}
			}
			None
		});

		LOG_BUFFER.push(LogEntry {
			level,
			target: event.metadata().target().to_string(),
			message,
			action_ctx,
		});
	}
}
