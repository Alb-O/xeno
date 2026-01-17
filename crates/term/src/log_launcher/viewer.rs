//! Interactive TUI log viewer with tracing-tree style output.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, ClearType};
use crossterm::{cursor, execute, queue};

use super::protocol::{Level, LogEvent, LogMessage, SpanEvent, XenoLayer};

const MAX_LOG_ENTRIES: usize = 10000;

/// A stored log entry (event or span lifecycle).
#[derive(Clone)]
enum StoredEntry {
	Event {
		event: LogEvent,
		relative_ms: u64,
	},
	SpanEnter {
		name: String,
		target: String,
		level: Level,
		layer: XenoLayer,
		fields: Vec<(String, String)>,
		depth: usize,
		relative_ms: u64,
	},
	SpanClose {
		name: String,
		level: Level,
		layer: XenoLayer,
		duration_us: u64,
		depth: usize,
	},
}

impl StoredEntry {
	fn level(&self) -> Level {
		match self {
			StoredEntry::Event { event, .. } => event.level,
			StoredEntry::SpanEnter { level, .. } | StoredEntry::SpanClose { level, .. } => *level,
		}
	}

	fn layer(&self) -> XenoLayer {
		match self {
			StoredEntry::Event { event, .. } => event.layer,
			StoredEntry::SpanEnter { layer, .. } | StoredEntry::SpanClose { layer, .. } => *layer,
		}
	}

	fn target(&self) -> &str {
		match self {
			StoredEntry::Event { event, .. } => &event.target,
			StoredEntry::SpanEnter { target, .. } => target,
			StoredEntry::SpanClose { .. } => "",
		}
	}
}

/// Tracked editor statistics from editor.stats events.
#[derive(Debug, Default, Clone)]
struct EditorStats {
	hooks_pending: u64,
	hooks_scheduled: u64,
	hooks_completed: u64,
	lsp_pending_docs: u64,
	lsp_in_flight: u64,
	lsp_full_sync: u64,
	lsp_incremental_sync: u64,
	lsp_send_errors: u64,
	last_updated: Option<Instant>,
}

impl EditorStats {
	fn update_from_fields(&mut self, fields: &[(String, String)]) {
		for (key, value) in fields {
			if let Ok(v) = value.parse::<u64>() {
				match key.as_str() {
					"hooks_pending" => self.hooks_pending = v,
					"hooks_scheduled" => self.hooks_scheduled = v,
					"hooks_completed" => self.hooks_completed = v,
					"lsp_pending_docs" => self.lsp_pending_docs = v,
					"lsp_in_flight" => self.lsp_in_flight = v,
					"lsp_full_sync" => self.lsp_full_sync = v,
					"lsp_incremental_sync" => self.lsp_incremental_sync = v,
					"lsp_send_errors" => self.lsp_send_errors = v,
					_ => {}
				}
			}
		}
		self.last_updated = Some(Instant::now());
	}
}

pub struct LogViewer {
	entries: VecDeque<StoredEntry>,
	active_spans: HashMap<u64, ActiveSpan>,
	start_time: Instant,
	min_level: Level,
	layer_filter: HashSet<XenoLayer>,
	target_filter: String,
	paused: bool,
	scroll_offset: usize,
	auto_scroll: bool,
	term_height: u16,
	term_width: u16,
	show_help: bool,
	show_stats: bool,
	stats: EditorStats,
	dirty: bool,
	disconnected: bool,
}

struct ActiveSpan {
	name: String,
	#[allow(dead_code)]
	target: String,
	level: Level,
	layer: XenoLayer,
	depth: usize,
}

const HEADER_WIDTH: &str = "       ";
const PIPE: &str = "\x1b[90m│\x1b[0m ";

impl LogViewer {
	pub fn new() -> Self {
		let (width, height) = terminal::size().unwrap_or((80, 24));
		Self {
			entries: VecDeque::with_capacity(MAX_LOG_ENTRIES),
			active_spans: HashMap::new(),
			start_time: Instant::now(),
			min_level: Level::Trace,
			layer_filter: HashSet::new(),
			target_filter: String::new(),
			paused: false,
			scroll_offset: 0,
			auto_scroll: true,
			term_height: height,
			term_width: width,
			show_help: false,
			show_stats: false,
			stats: EditorStats::default(),
			dirty: true,
			disconnected: false,
		}
	}

	fn matches_filter(&self, entry: &StoredEntry) -> bool {
		// Filter out log crate facade noise (target is literally "log")
		if entry.target() == "log" {
			return false;
		}
		if entry.level() < self.min_level {
			return false;
		}
		if !self.layer_filter.is_empty() && !self.layer_filter.contains(&entry.layer()) {
			return false;
		}
		if !self.target_filter.is_empty() && !entry.target().contains(&self.target_filter) {
			return false;
		}
		true
	}

	fn toggle_layer(&mut self, layer: XenoLayer) {
		if self.layer_filter.contains(&layer) {
			self.layer_filter.remove(&layer);
		} else {
			self.layer_filter.insert(layer);
		}
	}

	fn clear_layer_filter(&mut self) {
		self.layer_filter.clear();
	}

	fn push_entry(&mut self, entry: StoredEntry) {
		if self.entries.len() >= MAX_LOG_ENTRIES {
			self.entries.pop_front();
		}
		self.entries.push_back(entry);
		if self.auto_scroll {
			self.scroll_offset = 0;
		}
	}

	fn handle_message(&mut self, msg: LogMessage) {
		let relative_ms = self.start_time.elapsed().as_millis() as u64;
		match msg {
			LogMessage::Event(ref event) => {
				if event.message == "editor.stats" {
					self.stats.update_from_fields(&event.fields);
				}
				self.push_entry(StoredEntry::Event {
					event: event.clone(),
					relative_ms,
				});
			}
			LogMessage::Span(span_event) => self.handle_span(span_event, relative_ms),
			LogMessage::Disconnected => self.disconnected = true,
		}
		self.dirty = true;
	}

	fn handle_span(&mut self, span_event: SpanEvent, relative_ms: u64) {
		match span_event {
			SpanEvent::Enter {
				id,
				name,
				target,
				level,
				layer,
				fields,
				parent_id,
			} => {
				let depth = parent_id
					.and_then(|pid| self.active_spans.get(&pid).map(|s| s.depth + 1))
					.unwrap_or(0);

				self.push_entry(StoredEntry::SpanEnter {
					name: name.clone(),
					target: target.clone(),
					level,
					layer,
					fields,
					depth,
					relative_ms,
				});

				self.active_spans.insert(
					id,
					ActiveSpan {
						name,
						target,
						level,
						layer,
						depth,
					},
				);
			}
			SpanEvent::Exit { .. } => {}
			SpanEvent::Close { id, duration_us } => {
				if let Some(span) = self.active_spans.remove(&id) {
					self.push_entry(StoredEntry::SpanClose {
						name: span.name,
						level: span.level,
						layer: span.layer,
						duration_us,
						depth: span.depth,
					});
				}
			}
		}
	}

	fn format_entry(&self, entry: &StoredEntry) -> Vec<String> {
		match entry {
			StoredEntry::Event { event, relative_ms } => {
				let indent = self.line_prefix(event.spans.len(), false);
				let cont = self.line_prefix(event.spans.len(), true);
				let ts = format_relative_time(*relative_ms);
				let target = truncate_target(&event.target);
				let mut lines = vec![format!(
					"{}{} {} {}",
					indent,
					dim(&ts),
					event.level.colored(),
					event.layer.colored()
				)];
				for (i, msg_line) in event.message.lines().enumerate() {
					if i == 0 {
						lines.push(format!("{}{} > {}", cont, dim(&target), msg_line));
					} else {
						lines.push(format!("{}  {}", cont, msg_line));
					}
				}
				let fields: Vec<_> = event
					.fields
					.iter()
					.filter(|(k, _)| !k.starts_with("log."))
					.collect();
				let max_key = fields.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
				for (key, value) in fields {
					let value = value.replace('\n', " ");
					lines.push(format!(
						"{}    {} = {}",
						cont,
						dim(&format!("{:>width$}", key, width = max_key)),
						value
					));
				}
				lines
			}
			StoredEntry::SpanEnter {
				name,
				target,
				level,
				layer,
				fields,
				depth,
				relative_ms,
			} => {
				let indent = self.line_prefix(*depth, false);
				let cont = self.line_prefix(*depth, true);
				let ts = format_relative_time(*relative_ms);
				let target = truncate_target(target);
				let mut lines = vec![
					format!(
						"{}{} {} {}",
						indent,
						dim(&ts),
						level.colored(),
						layer.colored()
					),
					format!("{}{} {}", cont, dim(&target), cyan(name)),
				];
				let fields: Vec<_> = fields
					.iter()
					.filter(|(k, _)| !k.starts_with("log."))
					.collect();
				let max_key = fields.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
				for (key, value) in fields {
					let value = value.replace('\n', " ");
					lines.push(format!(
						"{}    {} = {}",
						cont,
						dim(&format!("{:>width$}", key, width = max_key)),
						value
					));
				}
				lines
			}
			StoredEntry::SpanClose {
				name,
				duration_us,
				depth,
				..
			} => {
				let duration = format_duration(*duration_us);
				let prefix = self.line_prefix(*depth, true);
				vec![format!("{}← {} {}", prefix, cyan(name), dim(&duration))]
			}
		}
	}

	fn render(&mut self, stdout: &mut io::Stdout) -> io::Result<()> {
		queue!(stdout, cursor::MoveTo(0, 0))?;

		let content_height = self.term_height.saturating_sub(1) as usize;
		let all_lines: Vec<String> = self
			.entries
			.iter()
			.filter(|e| self.matches_filter(e))
			.flat_map(|e| self.format_entry(e))
			.collect();

		let total_lines = all_lines.len();
		let visible_entries = self
			.entries
			.iter()
			.filter(|e| self.matches_filter(e))
			.count();

		if total_lines == 0 {
			for row in 0..content_height {
				queue!(stdout, cursor::MoveTo(0, row as u16))?;
				queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;
			}
			self.render_status_bar(stdout, visible_entries)?;
			return stdout.flush();
		}

		let max_offset = total_lines.saturating_sub(content_height);
		self.scroll_offset = self.scroll_offset.min(max_offset);

		let start = max_offset.saturating_sub(self.scroll_offset);
		let end = (start + content_height).min(total_lines);

		for (row, line) in all_lines[start..end].iter().enumerate() {
			queue!(stdout, cursor::MoveTo(0, row as u16))?;
			queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;
			write!(stdout, "{}", line)?;
		}

		for row in (end - start)..content_height {
			queue!(stdout, cursor::MoveTo(0, row as u16))?;
			queue!(stdout, terminal::Clear(ClearType::CurrentLine))?;
		}

		self.render_status_bar(stdout, visible_entries)?;
		if self.show_help {
			self.render_help(stdout)?;
		}
		if self.show_stats {
			self.render_stats(stdout)?;
		}

		stdout.flush()
	}

	fn line_prefix(&self, depth: usize, continuation: bool) -> String {
		if depth == 0 && !continuation {
			return String::new();
		}
		let mut prefix = String::from(HEADER_WIDTH);
		for i in 0..depth {
			prefix.push_str(PIPE);
			if i < depth - 1 || continuation {
				prefix.push_str(HEADER_WIDTH);
			}
		}
		if continuation {
			prefix.push_str(PIPE);
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
		let status = if self.disconnected {
			"\x1b[31m EXITED \x1b[0m\x1b[7m "
		} else if self.paused {
			"PAUSED "
		} else {
			""
		};
		let scroll = if self.scroll_offset > 0 {
			format!(" [+{}]", self.scroll_offset)
		} else {
			String::new()
		};
		let layer_str = if self.layer_filter.is_empty() {
			String::new()
		} else {
			let layers: Vec<_> = self.layer_filter.iter().map(|l| l.short_name()).collect();
			format!(" [{}]", layers.join(","))
		};
		let filter = if self.target_filter.is_empty() {
			String::new()
		} else {
			format!(" filter:{}", self.target_filter)
		};

		write!(
			stdout,
			"\x1b[7m {}{} {} lines{}{}{} | ? help | q quit \x1b[0m",
			status, level, total_lines, layer_str, scroll, filter
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
			"  Layer Filters (toggle):",
			"    1 CORE  2 API   3 LSP   4 LANG  5 CFG",
			"    6 UI    7 REG   8 EXT",
			"    L       Clear layer filter (show all)",
			"",
			"  Navigation:",
			"    j/Down    Scroll down",
			"    k/Up      Scroll up",
			"    g/Home    Go to top",
			"    G/End     Go to bottom",
			"    Space     Toggle pause",
			"",
			"  Other:",
			"    s         Toggle stats overlay",
			"    c         Clear logs",
			"    /         Set target filter",
			"    Esc       Clear filter / close help",
			"    q         Quit",
			"",
		];

		let box_width = 45usize;
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

	fn render_stats(&self, stdout: &mut io::Stdout) -> io::Result<()> {
		let age = self
			.stats
			.last_updated
			.map(|t| {
				let secs = t.elapsed().as_secs();
				if secs < 60 {
					format!("{}s ago", secs)
				} else {
					format!("{}m ago", secs / 60)
				}
			})
			.unwrap_or_else(|| "never".to_string());

		let lines: Vec<String> = vec![
			String::new(),
			"  Editor Statistics".to_string(),
			"  ─────────────────".to_string(),
			String::new(),
			"  Hooks:".to_string(),
			format!("    Pending:   {:>8}", self.stats.hooks_pending),
			format!("    Scheduled: {:>8}", self.stats.hooks_scheduled),
			format!("    Completed: {:>8}", self.stats.hooks_completed),
			String::new(),
			"  LSP Sync:".to_string(),
			format!("    Pending:     {:>6}", self.stats.lsp_pending_docs),
			format!("    In-flight:   {:>6}", self.stats.lsp_in_flight),
			format!("    Full sync:   {:>6}", self.stats.lsp_full_sync),
			format!("    Incremental: {:>6}", self.stats.lsp_incremental_sync),
			format!("    Errors:      {:>6}", self.stats.lsp_send_errors),
			String::new(),
			format!("  Updated: {}", age),
			String::new(),
		];

		let box_width = 30usize;
		let start_col = self.term_width.saturating_sub(box_width as u16 + 2);
		let start_row = 1u16;

		for (i, line) in lines.iter().enumerate() {
			queue!(stdout, cursor::MoveTo(start_col, start_row + i as u16))?;
			write!(
				stdout,
				"\x1b[42;30m{:width$}\x1b[0m",
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
		if self.show_stats {
			self.show_stats = false;
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
			KeyCode::Char('s') => self.show_stats = !self.show_stats,
			KeyCode::Char('t') => self.min_level = Level::Trace,
			KeyCode::Char('d') => self.min_level = Level::Debug,
			KeyCode::Char('i') => self.min_level = Level::Info,
			KeyCode::Char('w') => self.min_level = Level::Warn,
			KeyCode::Char('e') => self.min_level = Level::Error,
			KeyCode::Char('1') => self.toggle_layer(XenoLayer::Core),
			KeyCode::Char('2') => self.toggle_layer(XenoLayer::Api),
			KeyCode::Char('3') => self.toggle_layer(XenoLayer::Lsp),
			KeyCode::Char('4') => self.toggle_layer(XenoLayer::Lang),
			KeyCode::Char('5') => self.toggle_layer(XenoLayer::Config),
			KeyCode::Char('6') => self.toggle_layer(XenoLayer::Ui),
			KeyCode::Char('7') => self.toggle_layer(XenoLayer::Registry),
			KeyCode::Char('8') => self.toggle_layer(XenoLayer::External),
			KeyCode::Char('L') => self.clear_layer_filter(),
			KeyCode::Char('j') | KeyCode::Down => {
				self.scroll_offset = self.scroll_offset.saturating_sub(1);
			}
			KeyCode::Char('k') | KeyCode::Up => {
				self.scroll_offset += 1;
				self.auto_scroll = false;
			}
			KeyCode::Char('g') | KeyCode::Home => {
				self.scroll_offset = usize::MAX;
				self.auto_scroll = false;
			}
			KeyCode::Char('G') | KeyCode::End => {
				self.scroll_offset = 0;
				self.auto_scroll = true;
			}
			KeyCode::Char(' ') => self.paused = !self.paused,
			KeyCode::Char('c') => {
				self.entries.clear();
				self.active_spans.clear();
				self.scroll_offset = 0;
			}
			KeyCode::Esc => {
				self.target_filter.clear();
				self.clear_layer_filter();
			}
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
						let _ = tx.send(LogMessage::Disconnected);
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

fn format_duration(us: u64) -> String {
	if us > 1_000_000 {
		format!("{:.2}s", us as f64 / 1_000_000.0)
	} else if us > 1_000 {
		format!("{:.2}ms", us as f64 / 1_000.0)
	} else {
		format!("{}us", us)
	}
}

fn format_relative_time(ms: u64) -> String {
	if ms >= 60_000 {
		format!("{:>2}:{:02}", ms / 60_000, (ms % 60_000) / 1000)
	} else {
		format!("{:>5.1}s", ms as f64 / 1000.0)
	}
}

/// Strips `xeno_*` crate prefix from target paths.
///
/// Examples: `xeno_lsp::registry` → `registry`, `xeno_api::editor::ops` → `editor::ops`
fn truncate_target(target: &str) -> String {
	if let Some(rest) = target.strip_prefix("xeno_")
		&& let Some(pos) = rest.find("::")
	{
		return rest[pos + 2..].to_string();
	}
	target.to_string()
}
