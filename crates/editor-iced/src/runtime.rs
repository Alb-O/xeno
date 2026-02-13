use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::{column, container, scrollable, text};
use iced::{Element, Event, Fill, Font, Subscription, Task, event, keyboard, time, window};
use xeno_editor::runtime::{CursorStyle, LoopDirective, RuntimeEvent};
use xeno_editor::{Buffer, Editor};
use xeno_primitives::{Key, KeyCode, Modifiers};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(16);
const MAX_VISIBLE_BUFFER_LINES: usize = 500;
const DEFAULT_CELL_WIDTH_PX: f32 = 8.0;
const DEFAULT_CELL_HEIGHT_PX: f32 = 16.0;

#[derive(Debug, Clone, Default)]
pub struct StartupOptions {
	pub path: Option<PathBuf>,
	pub theme: Option<String>,
}

impl StartupOptions {
	pub fn from_env() -> Self {
		let mut path: Option<PathBuf> = None;
		let mut theme: Option<String> = None;
		let mut args = std::env::args().skip(1);

		while let Some(arg) = args.next() {
			if arg == "--theme" {
				theme = args.next();
				continue;
			}
			if path.is_none() {
				path = Some(PathBuf::from(arg));
			}
		}

		Self { path, theme }
	}
}

#[derive(Debug, Clone)]
enum Message {
	Tick(time::Instant),
	Event(Event),
}

#[derive(Debug, Default)]
struct Snapshot {
	title: String,
	header: String,
	statusline: String,
	body: String,
}

struct IcedEditorApp {
	runtime: tokio::runtime::Runtime,
	editor: Editor,
	directive: LoopDirective,
	quit_hook_emitted: bool,
	snapshot: Snapshot,
	cell_metrics: CellMetrics,
}

#[derive(Debug, Clone, Copy)]
struct CellMetrics {
	width_px: f32,
	height_px: f32,
}

impl CellMetrics {
	fn from_env() -> Self {
		Self {
			width_px: parse_cell_size(std::env::var("XENO_ICED_CELL_WIDTH_PX").ok(), DEFAULT_CELL_WIDTH_PX),
			height_px: parse_cell_size(std::env::var("XENO_ICED_CELL_HEIGHT_PX").ok(), DEFAULT_CELL_HEIGHT_PX),
		}
	}

	fn to_grid(self, logical_width_px: f32, logical_height_px: f32) -> (u16, u16) {
		(
			logical_pixels_to_cells(logical_width_px, self.width_px),
			logical_pixels_to_cells(logical_height_px, self.height_px),
		)
	}
}

impl IcedEditorApp {
	fn boot(startup: StartupOptions) -> (Self, Task<Message>) {
		xeno_editor::bootstrap::init();

		let runtime = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("tokio runtime for iced frontend");

		let user_config = Editor::load_user_config();

		let mut editor = match startup.path {
			Some(path) => Editor::new_with_path(path),
			None => Editor::new_scratch(),
		};

		editor.kick_theme_load();
		editor.kick_lsp_catalog_load();
		editor.apply_loaded_config(user_config);

		if let Some(theme_name) = startup.theme {
			editor.set_configured_theme_name(theme_name);
		}

		editor.ui_startup();
		editor.emit_editor_start_hook();

		let mut app = Self {
			runtime,
			editor,
			directive: default_loop_directive(),
			quit_hook_emitted: false,
			snapshot: Snapshot::default(),
			cell_metrics: CellMetrics::from_env(),
		};

		app.directive = app.runtime.block_on(app.editor.pump());
		app.rebuild_snapshot();

		(app, Task::none())
	}

	fn update(&mut self, message: Message) -> Task<Message> {
		match message {
			Message::Tick(_now) => {
				self.directive = self.runtime.block_on(self.editor.pump());
				self.rebuild_snapshot();
			}
			Message::Event(event) => {
				if matches!(event, Event::Window(window::Event::CloseRequested)) {
					self.directive.should_quit = true;
				} else if let Some(runtime_event) = map_event(event, self.cell_metrics) {
					self.directive = self.runtime.block_on(self.editor.on_event(runtime_event));
					self.rebuild_snapshot();
				}
			}
		}

		if self.directive.should_quit {
			self.emit_quit_hook_once();
			return iced::exit();
		}

		Task::none()
	}

	fn view(&self) -> Element<'_, Message> {
		let content = column![
			text(&self.snapshot.header).font(Font::MONOSPACE),
			text(&self.snapshot.statusline).font(Font::MONOSPACE),
			scrollable(text(&self.snapshot.body).font(Font::MONOSPACE)).height(Fill),
		]
		.spacing(8)
		.padding(12);

		container(content).into()
	}

	fn subscription(&self) -> Subscription<Message> {
		let mut tick_interval = self.directive.poll_timeout.unwrap_or(DEFAULT_POLL_INTERVAL);
		if tick_interval.is_zero() {
			tick_interval = DEFAULT_POLL_INTERVAL;
		}

		Subscription::batch([event::listen().map(Message::Event), time::every(tick_interval).map(Message::Tick)])
	}

	fn title(&self) -> String {
		self.snapshot.title.clone()
	}

	fn emit_quit_hook_once(&mut self) {
		if self.quit_hook_emitted {
			return;
		}
		self.runtime.block_on(self.editor.emit_editor_quit_hook());
		self.quit_hook_emitted = true;
	}

	fn rebuild_snapshot(&mut self) {
		self.editor.ensure_syntax_for_buffers();

		let mode = self.editor.mode_name();
		let cursor_line = self.editor.cursor_line() + 1;
		let cursor_col = self.editor.cursor_col() + 1;
		let buffers = self.editor.buffer_count();
		let focused = self.editor.focused_view();
		let statusline = self
			.editor
			.statusline_render_plan()
			.into_iter()
			.map(|segment| segment.text)
			.collect::<Vec<_>>()
			.join("");

		let (title, body) = self.editor.get_buffer(focused).map_or_else(
			|| (String::from("xeno-iced"), String::from("no focused buffer")),
			|buffer| snapshot_for_buffer(buffer),
		);

		self.snapshot = Snapshot {
			title,
			header: format!("mode={mode} cursor={cursor_line}:{cursor_col} buffers={buffers}"),
			statusline,
			body,
		};

		self.editor.frame_mut().needs_redraw = false;
	}
}

fn default_loop_directive() -> LoopDirective {
	LoopDirective {
		poll_timeout: Some(DEFAULT_POLL_INTERVAL),
		needs_redraw: true,
		cursor_style: CursorStyle::Block,
		should_quit: false,
	}
}

fn snapshot_for_buffer(buffer: &Buffer) -> (String, String) {
	let path = buffer.path();
	let modified = buffer.modified();
	let readonly = buffer.is_readonly();
	let start_line = buffer.scroll_line;

	let title = path
		.as_ref()
		.map(|path| format!("xeno-iced - {}", path.display()))
		.unwrap_or_else(|| String::from("xeno-iced - [scratch]"));

	let mut body = String::new();

	buffer.with_doc(|doc| {
		let content = doc.content();
		let total_lines = content.len_lines();
		let start = start_line.min(total_lines.saturating_sub(1));
		let end = start.saturating_add(MAX_VISIBLE_BUFFER_LINES).min(total_lines);

		let _ = writeln!(
			&mut body,
			"path={} modified={} readonly={} lines={} showing={}..{}",
			path.as_ref().map_or_else(|| String::from("[scratch]"), |path| path.display().to_string()),
			modified,
			readonly,
			total_lines,
			start + 1,
			end,
		);
		let _ = writeln!(&mut body);

		for line_idx in start..end {
			let line = content.line(line_idx).to_string();
			let line = line.trim_end_matches(['\n', '\r']);
			let _ = writeln!(&mut body, "{:>6} {line}", line_idx + 1);
		}

		if end < total_lines {
			let remaining = total_lines.saturating_sub(end);
			let _ = writeln!(&mut body);
			let _ = writeln!(&mut body, "... {remaining} more lines not shown");
		}
	});

	(title, body)
}

fn map_event(event: Event, cell_metrics: CellMetrics) -> Option<RuntimeEvent> {
	match event {
		Event::Keyboard(keyboard::Event::KeyPressed {
			modified_key,
			physical_key,
			modifiers,
			..
		}) => map_key_event(modified_key, physical_key, modifiers).map(RuntimeEvent::Key),
		Event::Window(window::Event::Opened { size, .. }) | Event::Window(window::Event::Resized(size)) => {
			let (cols, rows) = cell_metrics.to_grid(size.width, size.height);
			Some(RuntimeEvent::WindowResized { cols, rows })
		}
		Event::Window(window::Event::Focused) => Some(RuntimeEvent::FocusIn),
		Event::Window(window::Event::Unfocused) => Some(RuntimeEvent::FocusOut),
		_ => None,
	}
}

fn logical_pixels_to_cells(logical_px: f32, cell_px: f32) -> u16 {
	if !logical_px.is_finite() || !cell_px.is_finite() || cell_px <= 0.0 {
		return 1;
	}

	let cells = (logical_px / cell_px).floor();
	cells.clamp(1.0, u16::MAX as f32) as u16
}

fn parse_cell_size(value: Option<String>, default: f32) -> f32 {
	let Some(value) = value else {
		return default;
	};

	match value.parse::<f32>() {
		Ok(px) if px.is_finite() && px > 0.0 => px,
		_ => default,
	}
}

fn map_key_event(key: keyboard::Key, physical_key: keyboard::key::Physical, modifiers: keyboard::Modifiers) -> Option<Key> {
	let modifiers = Modifiers {
		ctrl: modifiers.control(),
		alt: modifiers.alt(),
		shift: modifiers.shift(),
	};

	let code = match key.as_ref() {
		keyboard::Key::Character(chars) => {
			let mut it = chars.chars();
			let ch = it.next().or_else(|| key.to_latin(physical_key))?;
			if it.next().is_some() {
				return None;
			}
			KeyCode::Char(ch)
		}
		keyboard::Key::Named(named) => map_named_key(named)?,
		keyboard::Key::Unidentified => return None,
	};

	Some(Key { code, modifiers })
}

fn map_named_key(key: keyboard::key::Named) -> Option<KeyCode> {
	use keyboard::key::Named;

	match key {
		Named::ArrowDown => Some(KeyCode::Down),
		Named::ArrowLeft => Some(KeyCode::Left),
		Named::ArrowRight => Some(KeyCode::Right),
		Named::ArrowUp => Some(KeyCode::Up),
		Named::Backspace => Some(KeyCode::Backspace),
		Named::Delete => Some(KeyCode::Delete),
		Named::End => Some(KeyCode::End),
		Named::Enter => Some(KeyCode::Enter),
		Named::Escape => Some(KeyCode::Esc),
		Named::Home => Some(KeyCode::Home),
		Named::Insert => Some(KeyCode::Insert),
		Named::PageDown => Some(KeyCode::PageDown),
		Named::PageUp => Some(KeyCode::PageUp),
		Named::Space => Some(KeyCode::Space),
		Named::Tab => Some(KeyCode::Tab),
		Named::F1 => Some(KeyCode::F(1)),
		Named::F2 => Some(KeyCode::F(2)),
		Named::F3 => Some(KeyCode::F(3)),
		Named::F4 => Some(KeyCode::F(4)),
		Named::F5 => Some(KeyCode::F(5)),
		Named::F6 => Some(KeyCode::F(6)),
		Named::F7 => Some(KeyCode::F(7)),
		Named::F8 => Some(KeyCode::F(8)),
		Named::F9 => Some(KeyCode::F(9)),
		Named::F10 => Some(KeyCode::F(10)),
		Named::F11 => Some(KeyCode::F(11)),
		Named::F12 => Some(KeyCode::F(12)),
		Named::F13 => Some(KeyCode::F(13)),
		Named::F14 => Some(KeyCode::F(14)),
		Named::F15 => Some(KeyCode::F(15)),
		Named::F16 => Some(KeyCode::F(16)),
		Named::F17 => Some(KeyCode::F(17)),
		Named::F18 => Some(KeyCode::F(18)),
		Named::F19 => Some(KeyCode::F(19)),
		Named::F20 => Some(KeyCode::F(20)),
		Named::F21 => Some(KeyCode::F(21)),
		Named::F22 => Some(KeyCode::F(22)),
		Named::F23 => Some(KeyCode::F(23)),
		Named::F24 => Some(KeyCode::F(24)),
		Named::F25 => Some(KeyCode::F(25)),
		Named::F26 => Some(KeyCode::F(26)),
		Named::F27 => Some(KeyCode::F(27)),
		Named::F28 => Some(KeyCode::F(28)),
		Named::F29 => Some(KeyCode::F(29)),
		Named::F30 => Some(KeyCode::F(30)),
		Named::F31 => Some(KeyCode::F(31)),
		Named::F32 => Some(KeyCode::F(32)),
		Named::F33 => Some(KeyCode::F(33)),
		Named::F34 => Some(KeyCode::F(34)),
		Named::F35 => Some(KeyCode::F(35)),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn logical_pixels_to_cells_uses_floor_mapping() {
		assert_eq!(logical_pixels_to_cells(79.9, 8.0), 9);
		assert_eq!(logical_pixels_to_cells(80.0, 8.0), 10);
	}

	#[test]
	fn logical_pixels_to_cells_clamps_minimum_to_one_cell() {
		assert_eq!(logical_pixels_to_cells(0.0, 8.0), 1);
		assert_eq!(logical_pixels_to_cells(-10.0, 8.0), 1);
	}

	#[test]
	fn parse_cell_size_falls_back_for_invalid_values() {
		assert_eq!(parse_cell_size(Some(String::from("abc")), 8.0), 8.0);
		assert_eq!(parse_cell_size(Some(String::from("0")), 8.0), 8.0);
		assert_eq!(parse_cell_size(Some(String::from("-4")), 8.0), 8.0);
		assert_eq!(parse_cell_size(None, 8.0), 8.0);
	}
}

pub fn run(startup: StartupOptions) -> iced::Result {
	configure_linux_backend();

	iced::application(move || IcedEditorApp::boot(startup.clone()), IcedEditorApp::update, IcedEditorApp::view)
		.title(IcedEditorApp::title)
		.subscription(IcedEditorApp::subscription)
		.window_size((1200.0, 800.0))
		.run()
}

#[cfg(target_os = "linux")]
fn configure_linux_backend() {
	if std::env::var_os("WINIT_UNIX_BACKEND").is_some() {
		return;
	}

	if let Some(requested) = std::env::var("XENO_ICED_BACKEND").ok().map(|value| value.to_lowercase()) {
		if matches!(requested.as_str(), "x11" | "wayland") {
			set_winit_unix_backend(&requested);
			return;
		}
	}

	if std::env::var_os("WAYLAND_DISPLAY").is_some() {
		set_winit_unix_backend("wayland");
		return;
	}

	if std::env::var_os("DISPLAY").is_some() {
		set_winit_unix_backend("x11");
	}
}

#[cfg(target_os = "linux")]
fn set_winit_unix_backend(value: &str) {
	unsafe {
		// SAFETY: This runs before iced/winit event-loop initialization and before
		// runtime task spawning, so no concurrent environment access occurs here.
		std::env::set_var("WINIT_UNIX_BACKEND", value);
	}
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_backend() {}
