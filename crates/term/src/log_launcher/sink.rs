//! Socket-based tracing layer that sends events to the log viewer.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::{Instant, SystemTime};

use parking_lot::Mutex;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::protocol::{Level, LogEvent, LogMessage, SpanEvent, SpanInfo, XenoLayer};

/// A tracing layer that sends log events over a Unix socket.
pub struct SocketLayer {
	socket: Mutex<UnixStream>,
}

impl SocketLayer {
	/// Creates a new socket layer connected to the given path.
	pub fn new(socket_path: &str) -> std::io::Result<Self> {
		let socket = UnixStream::connect(socket_path)?;
		socket.set_nonblocking(false)?;
		Ok(Self { socket: Mutex::new(socket) })
	}

	/// Sends a log message over the socket.
	fn send(&self, msg: &LogMessage) {
		let mut socket = self.socket.lock();
		if let Ok(json) = serde_json::to_string(msg) {
			// Write length-prefixed JSON for framing
			let bytes = json.as_bytes();
			let len = bytes.len() as u32;
			let _ = socket.write_all(&len.to_le_bytes());
			let _ = socket.write_all(bytes);
		}
	}
}

/// Extension data stored on each span.
struct SpanData {
	name: String,
	target: String,
	level: Level,
	layer: XenoLayer,
	fields: Vec<(String, String)>,
	entered_at: Option<Instant>,
}

impl<S> Layer<S> for SocketLayer
where
	S: Subscriber + for<'a> LookupSpan<'a>,
{
	fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
		let span = ctx.span(id).expect("span must exist");
		let mut fields = Vec::new();
		let mut visitor = FieldVisitor(&mut fields);
		attrs.record(&mut visitor);

		let target = attrs.metadata().target();
		let data = SpanData {
			name: attrs.metadata().name().to_string(),
			target: target.to_string(),
			level: attrs.metadata().level().into(),
			layer: XenoLayer::from_target(target),
			fields,
			entered_at: None,
		};

		span.extensions_mut().insert(data);
	}

	fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
		if let Some(span) = ctx.span(id) {
			let mut extensions = span.extensions_mut();
			if let Some(data) = extensions.get_mut::<SpanData>() {
				let mut visitor = FieldVisitor(&mut data.fields);
				values.record(&mut visitor);
			}
		}
	}

	fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
		if let Some(span) = ctx.span(id) {
			let mut extensions = span.extensions_mut();
			if let Some(data) = extensions.get_mut::<SpanData>() {
				data.entered_at = Some(Instant::now());

				let parent_id = span.parent().map(|p| p.id().into_u64());
				self.send(&LogMessage::Span(SpanEvent::Enter {
					id: id.into_u64(),
					name: data.name.clone(),
					target: data.target.clone(),
					level: data.level,
					layer: data.layer,
					fields: data.fields.clone(),
					parent_id,
				}));
			}
		}
	}

	fn on_exit(&self, id: &Id, _ctx: Context<'_, S>) {
		self.send(&LogMessage::Span(SpanEvent::Exit { id: id.into_u64() }));
	}

	fn on_close(&self, id: Id, ctx: Context<'_, S>) {
		let duration_us = ctx
			.span(&id)
			.and_then(|span| {
				span.extensions()
					.get::<SpanData>()
					.and_then(|data| data.entered_at.map(|t| t.elapsed().as_micros() as u64))
			})
			.unwrap_or(0);

		self.send(&LogMessage::Span(SpanEvent::Close {
			id: id.into_u64(),
			duration_us,
		}));
	}

	fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
		let metadata = event.metadata();
		let target = metadata.target();

		// Collect fields
		let mut fields = Vec::new();
		let mut message = String::new();
		let mut visitor = EventVisitor {
			fields: &mut fields,
			message: &mut message,
		};
		event.record(&mut visitor);

		// Collect span context
		let spans: Vec<SpanInfo> = ctx
			.event_scope(event)
			.map(|scope| {
				scope
					.from_root()
					.map(|span| {
						let extensions = span.extensions();
						let data = extensions.get::<SpanData>();
						SpanInfo {
							name: span.name().to_string(),
							target: data.map(|d| d.target.clone()).unwrap_or_else(|| span.metadata().target().to_string()),
							fields: data.map(|d| d.fields.clone()).unwrap_or_default(),
						}
					})
					.collect()
			})
			.unwrap_or_default();

		let log_event = LogEvent {
			timestamp: SystemTime::now(),
			level: metadata.level().into(),
			layer: XenoLayer::from_target(target),
			target: target.to_string(),
			message,
			spans,
			fields,
		};

		self.send(&LogMessage::Event(log_event));
	}
}

/// Visitor that collects span fields.
struct FieldVisitor<'a>(&'a mut Vec<(String, String)>);

impl tracing::field::Visit for FieldVisitor<'_> {
	fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
		self.0.push((field.name().to_string(), format!("{:?}", value)));
	}

	fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
		self.0.push((field.name().to_string(), value.to_string()));
	}

	fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
		self.0.push((field.name().to_string(), value.to_string()));
	}

	fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
		self.0.push((field.name().to_string(), value.to_string()));
	}

	fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
		self.0.push((field.name().to_string(), value.to_string()));
	}
}

/// Visitor that collects event fields and extracts the message.
struct EventVisitor<'a> {
	fields: &'a mut Vec<(String, String)>,
	message: &'a mut String,
}

impl tracing::field::Visit for EventVisitor<'_> {
	fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
		if field.name() == "message" {
			*self.message = format!("{:?}", value);
		} else {
			self.fields.push((field.name().to_string(), format!("{:?}", value)));
		}
	}

	fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
		if field.name() == "message" {
			*self.message = value.to_string();
		} else {
			self.fields.push((field.name().to_string(), value.to_string()));
		}
	}

	fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
		self.fields.push((field.name().to_string(), value.to_string()));
	}

	fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
		self.fields.push((field.name().to_string(), value.to_string()));
	}

	fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
		self.fields.push((field.name().to_string(), value.to_string()));
	}
}
