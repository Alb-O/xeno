//! Interactive TUI log viewer with tracing-tree style output.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Read};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, ClearType};
use crossterm::{cursor, execute};

use super::protocol::{Level, LogMessage, SpanEvent, XenoLayer};

mod format;
mod render;
mod types;

use types::{ActiveSpan, EditorStats, MAX_LOG_ENTRIES, StoredEntry};

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
				let depth = parent_id.and_then(|pid| self.active_spans.get(&pid).map(|s| s.depth + 1)).unwrap_or(0);

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
			KeyCode::Char('q') | KeyCode::Char('C') if key.code == KeyCode::Char('q') || key.modifiers.contains(KeyModifiers::CONTROL) => {
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
	execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide, terminal::Clear(ClearType::All))?;

	let mut viewer = LogViewer::new();
	viewer.render(&mut stdout)?;

	let result = run_viewer_loop(&mut viewer, &rx, &mut stdout);

	execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
	terminal::disable_raw_mode()?;
	let _ = std::fs::remove_file(socket_path);

	result
}

fn run_viewer_loop(viewer: &mut LogViewer, rx: &Receiver<LogMessage>, stdout: &mut io::Stdout) -> io::Result<()> {
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
