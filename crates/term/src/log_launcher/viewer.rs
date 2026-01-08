//! Interactive TUI log viewer with tracing-tree style output.

use std::collections::{HashMap, VecDeque};

const FIELD_INDENT: &str = "    ";
const PIPE_PREFIX: &str = "\x1b[90m│\x1b[0m ";
use std::io::{self, Read, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, SystemTime};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, ClearType};
use crossterm::{cursor, execute, queue};

use super::protocol::{Level, LogEvent, LogMessage, SpanEvent, SpanInfo};

const MAX_LOG_ENTRIES: usize = 10000;

/// Log viewer state.
pub struct LogViewer {
	events: VecDeque<DisplayEntry>,
	active_spans: HashMap<u64, ActiveSpan>,
	min_level: Level,
	target_filter: String,
	paused: bool,
	scroll_offset: usize,
	auto_scroll: bool,
	term_height: u16,
	term_width: u16,
	show_help: bool,
	dirty: bool,
}

struct ActiveSpan {
	name: String,
	#[allow(dead_code)]
	target: String,
	level: Level,
	depth: usize,
}

#[derive(Clone)]
struct DisplayEntry {
	#[allow(dead_code)]
	timestamp: SystemTime,
	lines: Vec<FormattedLine>,
}

#[derive(Clone)]
struct FormattedLine {
	indent: usize,
	content: String,
	continuation: bool,
}

impl LogViewer {
	pub fn new() -> Self {
		let (width, height) = terminal::size().unwrap_or((80, 24));
		Self {
			events: VecDeque::with_capacity(MAX_LOG_ENTRIES),
			active_spans: HashMap::new(),
			min_level: Level::Trace,
			target_filter: String::new(),
			paused: false,
			scroll_offset: 0,
			auto_scroll: true,
			term_height: height,
			term_width: width,
			show_help: false,
			dirty: true,
		}
	}

	fn handle_message(&mut self, msg: LogMessage) {
		match msg {
			LogMessage::Event(event) => self.handle_event(event),
			LogMessage::Span(span_event) => self.handle_span(span_event),
		}
		self.dirty = true;
	}

	fn handle_event(&mut self, event: LogEvent) {
		if event.level < self.min_level {
			return;
		}
		if !self.target_filter.is_empty() && !event.target.contains(&self.target_filter) {
			return;
		}

		let entry = self.format_event(&event);
		if self.events.len() >= MAX_LOG_ENTRIES {
			self.events.pop_front();
		}
		self.events.push_back(entry);

		if self.auto_scroll {
			self.scroll_offset = 0;
		}
	}

	fn handle_span(&mut self, span_event: SpanEvent) {
		match span_event {
			SpanEvent::Enter {
				id,
				name,
				target,
				level,
				fields,
				parent_id,
			} => {
				let depth = parent_id
					.and_then(|pid| self.active_spans.get(&pid).map(|s| s.depth + 1))
					.unwrap_or(0);

				let entry = self.format_span_enter(&name, &target, level, &fields, depth);
				if level >= self.min_level {
					if self.events.len() >= MAX_LOG_ENTRIES {
						self.events.pop_front();
					}
					self.events.push_back(entry);
				}

				self.active_spans.insert(
					id,
					ActiveSpan {
						name,
						target,
						level,
						depth,
					},
				);
			}
			SpanEvent::Exit { .. } => {}
			SpanEvent::Close { id, duration_us } => {
				if let Some(span) = self.active_spans.remove(&id) {
					let entry = self.format_span_close(&span.name, duration_us, span.depth);
					if span.level >= self.min_level {
						if self.events.len() >= MAX_LOG_ENTRIES {
							self.events.pop_front();
						}
						self.events.push_back(entry);
					}
				}
			}
		}
	}

	fn format_event(&self, event: &LogEvent) -> DisplayEntry {
		let mut lines = Vec::new();
		let indent = self.calculate_indent(&event.spans);

		let content = format!(
			"{} {} > {}",
			event.level.colored(),
			dim(&event.target),
			&event.message
		);
		lines.push(FormattedLine {
			indent,
			content,
			continuation: false,
		});

		for (key, value) in &event.fields {
			lines.push(FormattedLine {
				indent,
				content: format!("{}{} = {}", FIELD_INDENT, dim(key), value),
				continuation: true,
			});
		}

		DisplayEntry {
			timestamp: event.timestamp,
			lines,
		}
	}

	fn format_span_enter(
		&self,
		name: &str,
		target: &str,
		level: Level,
		fields: &[(String, String)],
		depth: usize,
	) -> DisplayEntry {
		let mut lines = Vec::new();

		lines.push(FormattedLine {
			indent: depth,
			content: format!("{} {} {}", level.colored(), dim(target), cyan(name)),
			continuation: false,
		});

		for (key, value) in fields {
			lines.push(FormattedLine {
				indent: depth,
				content: format!("{}{} = {}", FIELD_INDENT, dim(key), value),
				continuation: true,
			});
		}

		DisplayEntry {
			timestamp: SystemTime::now(),
			lines,
		}
	}

	fn format_span_close(&self, name: &str, duration_us: u64, depth: usize) -> DisplayEntry {
		let duration = if duration_us > 1_000_000 {
			format!("{:.2}s", duration_us as f64 / 1_000_000.0)
		} else if duration_us > 1_000 {
			format!("{:.2}ms", duration_us as f64 / 1_000.0)
		} else {
			format!("{}us", duration_us)
		};

		DisplayEntry {
			timestamp: SystemTime::now(),
			lines: vec![FormattedLine {
				indent: depth,
				content: format!("{} {}", cyan(name), dim(&duration)),
				continuation: false,
			}],
		}
	}

	fn calculate_indent(&self, spans: &[SpanInfo]) -> usize {
		spans.len()
	}

	fn current_top_entry(&self) -> Option<usize> {
		if self.events.is_empty() {
			return None;
		}
		let total_lines: usize = self.events.iter().map(|e| e.lines.len()).sum();
		let content_height = self.term_height.saturating_sub(1) as usize;
		let max_offset = total_lines.saturating_sub(content_height);
		let scroll_offset = self.scroll_offset.min(max_offset);
		let start_line = max_offset.saturating_sub(scroll_offset);

		let mut line_count = 0;
		for (i, entry) in self.events.iter().enumerate() {
			if line_count + entry.lines.len() > start_line {
				return Some(i);
			}
			line_count += entry.lines.len();
		}
		Some(self.events.len() - 1)
	}

	fn render(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
		queue!(stdout, cursor::MoveTo(0, 0))?;

		let content_height = self.term_height.saturating_sub(1) as usize;
		let num_entries = self.events.len();

		if num_entries == 0 {
			for row in 0..content_height {
				queue!(stdout, cursor::MoveTo(0, row as u16))?;
				queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;
			}
			self.render_status_bar(stdout, 0)?;
			return stdout.flush();
		}

		let all_lines: Vec<&FormattedLine> = self.events.iter().flat_map(|e| &e.lines).collect();
		let total_lines = all_lines.len();

		// scroll_offset tracks lines scrolled up from bottom; 0 = showing newest
		let max_offset = total_lines.saturating_sub(content_height);
		self.scroll_offset = self.scroll_offset.min(max_offset);

		let start = max_offset.saturating_sub(self.scroll_offset);
		let end = (start + content_height).min(total_lines);

		for (row, line) in all_lines[start..end].iter().enumerate() {
			queue!(stdout, cursor::MoveTo(0, row as u16))?;
			queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;
			write!(
				stdout,
				"{}{}",
				self.line_prefix(line.indent, line.continuation),
				line.content
			)?;
		}

		for row in (end - start)..content_height {
			queue!(stdout, cursor::MoveTo(0, row as u16))?;
			queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;
		}

		self.render_status_bar(stdout, num_entries)?;
		if self.show_help {
			self.render_help(stdout)?;
		}

		stdout.flush()
	}

	fn line_prefix(&self, indent: usize, continuation: bool) -> String {
		if indent == 0 && !continuation {
			return String::new();
		}
		let mut prefix = String::new();
		for _ in 0..indent {
			prefix.push_str(PIPE_PREFIX);
		}
		if continuation {
			prefix.push_str(PIPE_PREFIX);
		}
		prefix
	}

	fn render_status_bar(&self, stdout: &mut io::Stdout, total_lines: usize) -> io::Result<()> {
		queue!(stdout, cursor::MoveTo(0, self.term_height - 1))?;
		queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;

		let level = match self.min_level {
			Level::Trace => "T",
			Level::Debug => "D",
			Level::Info => "I",
			Level::Warn => "W",
			Level::Error => "E",
		};
		let pause = if self.paused { "[PAUSED]" } else { "" };
		let scroll = if self.scroll_offset > 0 {
			format!(" [+{}]", self.scroll_offset)
		} else {
			String::new()
		};
		let filter = if self.target_filter.is_empty() {
			String::new()
		} else {
			format!(" filter:{}", self.target_filter)
		};

		write!(
			stdout,
			"\x1b[7m {}{} {} lines{}{} | ? help | q quit \x1b[0m",
			pause, level, total_lines, scroll, filter
		)
	}

	fn render_help(&self, stdout: &mut io::Stdout) -> io::Result<()> {
		const HELP: &[&str] = &[
			"",
			"  Log Viewer Controls",
			"  ───────────────────",
			"",
			"  Level Filters:",
			"    t  Show TRACE and above",
			"    d  Show DEBUG and above",
			"    i  Show INFO and above",
			"    w  Show WARN and above",
			"    e  Show ERROR only",
			"",
			"  Navigation:",
			"    j/Down    Scroll down",
			"    k/Up      Scroll up",
			"    g/Home    Go to top",
			"    G/End     Go to bottom",
			"    Space     Toggle pause",
			"",
			"  Other:",
			"    c         Clear logs",
			"    /         Set target filter",
			"    Esc       Clear filter / close help",
			"    q         Quit",
			"",
		];

		let box_width = 40usize;
		let start_col = (self.term_width / 2).saturating_sub(box_width as u16 / 2);
		let start_row = (self.term_height / 2).saturating_sub(HELP.len() as u16 / 2);

		for (i, line) in HELP.iter().enumerate() {
			queue!(stdout, cursor::MoveTo(start_col, start_row + i as u16))?;
			write!(
				stdout,
				"\x1b[44;97m{:width$}\x1b[0m",
				line,
				width = box_width
			)?;
		}

		Ok(())
	}

	fn handle_key(&mut self, key: KeyEvent) -> bool {
		if self.show_help {
			self.show_help = false;
			self.dirty = true;
			return false;
		}

		match key.code {
			KeyCode::Char('q') | KeyCode::Char('C')
				if key.code == KeyCode::Char('q')
					|| key.modifiers.contains(KeyModifiers::CONTROL) =>
			{
				return true;
			}
			KeyCode::Char('?') => self.show_help = true,
			KeyCode::Char('t') => self.min_level = Level::Trace,
			KeyCode::Char('d') => self.min_level = Level::Debug,
			KeyCode::Char('i') => self.min_level = Level::Info,
			KeyCode::Char('w') => self.min_level = Level::Warn,
			KeyCode::Char('e') => self.min_level = Level::Error,
			KeyCode::Char('j') | KeyCode::Down => {
				if let Some(entry) = self.current_top_entry()
					&& entry < self.events.len() - 1
				{
					self.scroll_offset = self
						.scroll_offset
						.saturating_sub(self.events[entry].lines.len());
				}
			}
			KeyCode::Char('k') | KeyCode::Up => {
				if let Some(entry) = self.current_top_entry()
					&& entry > 0
				{
					self.scroll_offset += self.events[entry - 1].lines.len();
				}
				self.auto_scroll = false;
			}
			KeyCode::Char('g') | KeyCode::Home => {
				let total_lines: usize = self.events.iter().map(|e| e.lines.len()).sum();
				self.scroll_offset =
					total_lines.saturating_sub(self.term_height.saturating_sub(1) as usize);
				self.auto_scroll = false;
			}
			KeyCode::Char('G') | KeyCode::End => {
				self.scroll_offset = 0;
				self.auto_scroll = true;
			}
			KeyCode::Char(' ') => self.paused = !self.paused,
			KeyCode::Char('c') => {
				self.events.clear();
				self.active_spans.clear();
				self.scroll_offset = 0;
			}
			KeyCode::Esc => self.target_filter.clear(),
			_ => return false,
		}
		self.dirty = true;
		false
	}

	fn handle_resize(&mut self, width: u16, height: u16) {
		self.term_width = width;
		self.term_height = height;
		self.dirty = true;
	}
}

/// Runs the log viewer, listening on the given socket path.
pub fn run_log_viewer(socket_path: &Path) -> io::Result<()> {
	let listener = UnixListener::bind(socket_path)?;
	listener.set_nonblocking(true)?;

	let (tx, rx): (mpsc::Sender<LogMessage>, Receiver<LogMessage>) = mpsc::channel();

	let _listener_handle = thread::spawn(move || {
		for stream in listener.incoming() {
			match stream {
				Ok(mut stream) => {
					let tx = tx.clone();
					thread::spawn(move || {
						let mut len_buf = [0u8; 4];
						loop {
							if stream.read_exact(&mut len_buf).is_err() {
								break;
							}
							let len = u32::from_le_bytes(len_buf) as usize;
							let mut json_buf = vec![0u8; len];
							if stream.read_exact(&mut json_buf).is_err() {
								break;
							}
							if let Ok(msg) = serde_json::from_slice::<LogMessage>(&json_buf)
								&& tx.send(msg).is_err()
							{
								break;
							}
						}
					});
				}
				Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
					thread::sleep(Duration::from_millis(10));
				}
				Err(_) => break,
			}
		}
	});

	terminal::enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(
		stdout,
		terminal::EnterAlternateScreen,
		cursor::Hide,
		terminal::Clear(ClearType::All)
	)?;

	let mut viewer = LogViewer::new();
	viewer.render(&mut stdout)?;

	let result = run_viewer_loop(&mut viewer, &rx, &mut stdout);

	execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
	terminal::disable_raw_mode()?;
	let _ = std::fs::remove_file(socket_path);

	result
}

fn run_viewer_loop(
	viewer: &mut LogViewer,
	rx: &Receiver<LogMessage>,
	stdout: &mut io::Stdout,
) -> io::Result<()> {
	loop {
		if !viewer.paused {
			while let Ok(msg) = rx.try_recv() {
				viewer.handle_message(msg);
			}
		}

		let timeout = if viewer.dirty {
			Duration::from_millis(16)
		} else {
			Duration::from_millis(100)
		};

		if event::poll(timeout)? {
			match event::read()? {
				Event::Key(key) if viewer.handle_key(key) => break,
				Event::Resize(w, h) => viewer.handle_resize(w, h),
				_ => {}
			}
		}

		if viewer.dirty {
			viewer.render(stdout)?;
			viewer.dirty = false;
		}
	}
	Ok(())
}

fn dim(s: &str) -> String {
	format!("\x1b[90m{}\x1b[0m", s)
}

fn cyan(s: &str) -> String {
	format!("\x1b[36m{}\x1b[0m", s)
}
