//! Startup splash screen with live log display.
//!
//! Shows a centered loading screen during editor initialization that displays
//! tracing log messages in real-time, giving visibility into startup progress.

use std::collections::VecDeque;
use std::io::{self, Write as _};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use termina::PlatformTerminal;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use xeno_tui::layout::Rect;
use xeno_tui::style::{Color, Style};
use xeno_tui::text::{Line, Span, Text};
use xeno_tui::widgets::{Block, Clear, Paragraph};
use xeno_tui::Terminal;

use crate::backend::TerminaBackend;

/// Maximum number of log entries to display.
const MAX_LOG_ENTRIES: usize = 12;

/// Delay before showing splash to avoid flashing on fast startups.
const SPLASH_DELAY: Duration = Duration::from_millis(300);

/// Captured log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
	pub level: Level,
	pub target: String,
	pub message: String,
}

/// Shared log entry buffer.
pub type LogBuffer = Arc<Mutex<VecDeque<LogEntry>>>;

/// Creates a new log buffer.
pub fn new_log_buffer() -> LogBuffer {
	Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_ENTRIES + 1)))
}

/// Tracing layer that captures events to a buffer for splash display.
pub struct SplashLogLayer {
	buffer: LogBuffer,
}

impl SplashLogLayer {
	/// Creates a layer writing to the given buffer.
	pub fn new(buffer: LogBuffer) -> Self {
		Self { buffer }
	}
}

impl<S> Layer<S> for SplashLogLayer
where
	S: Subscriber,
{
	fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
		let metadata = event.metadata();

		let mut message = String::new();
		let mut visitor = MessageVisitor(&mut message);
		event.record(&mut visitor);

		let entry = LogEntry {
			level: *metadata.level(),
			target: metadata.target().to_string(),
			message,
		};

		if let Ok(mut buf) = self.buffer.lock() {
			buf.push_back(entry);
			while buf.len() > MAX_LOG_ENTRIES {
				buf.pop_front();
			}
		}
	}
}

/// Extracts the `message` field from tracing events.
struct MessageVisitor<'a>(&'a mut String);

impl Visit for MessageVisitor<'_> {
	fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
		if field.name() == "message" {
			*self.0 = format!("{:?}", value);
		}
	}

	fn record_str(&mut self, field: &Field, value: &str) {
		if field.name() == "message" {
			*self.0 = value.to_string();
		}
	}
}

/// Renders the splash screen if at least 300ms has elapsed since `started_at`.
pub fn render_splash(
	terminal: &mut Terminal<TerminaBackend<PlatformTerminal>>,
	log_buffer: &LogBuffer,
	started_at: Instant,
) -> io::Result<()> {
	if started_at.elapsed() < SPLASH_DELAY {
		return Ok(());
	}

	terminal.draw(|frame| {
		let area = frame.area();
		frame.render_widget(Clear, area);

		let box_width = 60.min(area.width.saturating_sub(4));
		let box_height = (MAX_LOG_ENTRIES as u16 + 4).min(area.height.saturating_sub(2));
		let centered = center_rect(area, box_width, box_height);

		let mut lines: Vec<Line<'_>> = vec![
			Line::from(vec![
				Span::styled("xeno", Style::default().fg(Color::Cyan).bold()),
				Span::raw(" is starting..."),
			]),
			Line::raw(""),
		];

		if let Ok(buf) = log_buffer.lock() {
			let max_msg_len = box_width.saturating_sub(20) as usize;
			for entry in buf.iter() {
				let level_color = match entry.level {
					Level::ERROR => Color::Red,
					Level::WARN => Color::Yellow,
					Level::INFO => Color::Green,
					Level::DEBUG => Color::Blue,
					Level::TRACE => Color::DarkGray,
				};
				let short_target = entry.target.rsplit("::").next().unwrap_or(&entry.target);
				let msg = truncate_str(&entry.message, max_msg_len);

				lines.push(Line::from(vec![
					Span::styled(format!("{:5}", entry.level), Style::default().fg(level_color)),
					Span::raw(" "),
					Span::styled(
						format!("{:12}", truncate_str(short_target, 12)),
						Style::default().fg(Color::DarkGray),
					),
					Span::raw(" "),
					Span::raw(msg),
				]));
			}
		}

		lines.resize_with(box_height.saturating_sub(2) as usize, || Line::raw(""));

		let block = Block::bordered().border_style(Style::default().fg(Color::DarkGray));
		let paragraph = Paragraph::new(Text::from(lines)).block(block);
		frame.render_widget(paragraph, centered);
	})?;

	terminal.backend_mut().terminal_mut().flush()
}

/// Centers a rectangle within an area.
fn center_rect(area: Rect, width: u16, height: u16) -> Rect {
	let x = area.x + (area.width.saturating_sub(width)) / 2;
	let y = area.y + (area.height.saturating_sub(height)) / 2;
	Rect::new(x, y, width.min(area.width), height.min(area.height))
}

/// Truncates with ellipsis if exceeds `max_len`.
fn truncate_str(s: &str, max_len: usize) -> String {
	if s.len() <= max_len {
		s.to_string()
	} else if max_len > 2 {
		format!("{}...", &s[..max_len - 2])
	} else {
		s[..max_len].to_string()
	}
}
