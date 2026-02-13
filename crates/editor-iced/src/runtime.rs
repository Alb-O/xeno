use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::{column, container, scrollable, text};
use iced::{Element, Event, Fill, Font, Subscription, Task, event, keyboard, mouse, time, window};
use iced_core::input_method;
use xeno_editor::completion::CompletionRenderPlan;
use xeno_editor::geometry::Rect;
use xeno_editor::info_popup::{InfoPopupRenderAnchor, InfoPopupRenderTarget};
use xeno_editor::overlay::{OverlayControllerKind, OverlayPaneRenderTarget};
use xeno_editor::render_api::{BufferRenderContext, RenderLine};
use xeno_editor::runtime::{CursorStyle, LoopDirective, RuntimeEvent};
use xeno_editor::snippet::SnippetChoiceRenderPlan;
use xeno_editor::{Buffer, Editor, ViewId};
use xeno_primitives::{Key, KeyCode, Modifiers, MouseButton as CoreMouseButton, MouseEvent as CoreMouseEvent, ScrollDirection};

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
	ClipboardRead(Result<std::sync::Arc<String>, iced::clipboard::Error>),
}

#[derive(Debug, Default)]
struct Snapshot {
	title: String,
	header: String,
	statusline: String,
	surface_summary: String,
	completion_preview: String,
	snippet_preview: String,
	body: String,
}

struct IcedEditorApp {
	runtime: tokio::runtime::Runtime,
	editor: Editor,
	directive: LoopDirective,
	quit_hook_emitted: bool,
	snapshot: Snapshot,
	cell_metrics: CellMetrics,
	event_state: EventBridgeState,
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

#[derive(Debug, Clone, Default)]
struct EventBridgeState {
	mouse_row: u16,
	mouse_col: u16,
	mouse_button: Option<CoreMouseButton>,
	modifiers: Modifiers,
	ime_preedit: Option<String>,
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
			event_state: EventBridgeState::default(),
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
			Message::ClipboardRead(result) => {
				if let Ok(content) = result {
					self.directive = self.runtime.block_on(self.editor.on_event(RuntimeEvent::Paste(content.as_ref().clone())));
					self.rebuild_snapshot();
				}
			}
			Message::Event(event) => {
				if matches!(event, Event::Window(window::Event::CloseRequested)) {
					self.directive.should_quit = true;
				} else if let Some(task) = clipboard_paste_task(&event) {
					return task;
				} else if let Some(runtime_event) = map_event(event.clone(), self.cell_metrics, &mut self.event_state) {
					self.directive = self.runtime.block_on(self.editor.on_event(runtime_event));
					self.rebuild_snapshot();
				} else if matches!(event, Event::InputMethod(_)) {
					self.directive.needs_redraw = true;
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
			text(&self.snapshot.surface_summary).font(Font::MONOSPACE),
			text(&self.snapshot.completion_preview).font(Font::MONOSPACE),
			text(&self.snapshot.snippet_preview).font(Font::MONOSPACE),
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
		let overlay_kind = self.editor.overlay_kind();
		let overlay_panes = self.editor.overlay_pane_render_plan();
		let completion_plan = self.editor.completion_popup_render_plan();
		let snippet_plan = self.editor.snippet_choice_render_plan();
		let info_popup_plan = self.editor.info_popup_render_plan();
		let surface_summary = build_surface_summary(overlay_kind, &overlay_panes, completion_plan.as_ref(), snippet_plan.as_ref(), &info_popup_plan);
		let completion_preview = format_completion_preview(completion_plan.as_ref());
		let snippet_preview = format_snippet_preview(snippet_plan.as_ref());

		let (title, body) = snapshot_for_focused_view(&mut self.editor, focused).unwrap_or_else(|| {
			self.editor.get_buffer(focused).map_or_else(
				|| (String::from("xeno-iced"), String::from("no focused buffer")),
				|buffer| snapshot_for_buffer(buffer),
			)
		});

		self.snapshot = Snapshot {
			title,
			header: format!(
				"mode={mode} cursor={cursor_line}:{cursor_col} buffers={buffers} ime_preedit={}",
				ime_preedit_label(self.event_state.ime_preedit.as_deref())
			),
			statusline,
			surface_summary,
			completion_preview,
			snippet_preview,
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

fn snapshot_for_focused_view(editor: &mut Editor, focused: ViewId) -> Option<(String, String)> {
	let title = editor
		.get_buffer(focused)?
		.path()
		.as_ref()
		.map(|path| format!("xeno-iced - {}", path.display()))
		.unwrap_or_else(|| String::from("xeno-iced - [scratch]"));

	let area = editor.view_area(focused);
	if area.width < 2 || area.height == 0 {
		return None;
	}

	let render_ctx = editor.render_ctx();
	let mut cache = std::mem::take(editor.render_cache_mut());
	let tab_width = editor.tab_width_for(focused);
	let cursorline = editor.cursorline_for(focused);

	let body = editor.get_buffer(focused).map_or_else(
		|| String::from("no focused buffer"),
		|buffer| {
			let buffer_ctx = BufferRenderContext {
				theme: &render_ctx.theme,
				language_loader: &editor.config().language_loader,
				syntax_manager: editor.syntax_manager(),
				diagnostics: render_ctx.lsp.diagnostics_for(focused),
				diagnostic_ranges: render_ctx.lsp.diagnostic_ranges_for(focused),
			};

			let result = buffer_ctx.render_buffer(buffer, area, true, true, tab_width, cursorline, &mut cache);
			join_render_lines(result.gutter, result.text)
		},
	);

	*editor.render_cache_mut() = cache;
	Some((title, body))
}

fn join_render_lines(gutter: Vec<RenderLine<'static>>, text: Vec<RenderLine<'static>>) -> String {
	let row_count = gutter.len().max(text.len());
	let mut body = String::new();

	for idx in 0..row_count {
		let gutter_line = gutter.get(idx).map_or_else(String::new, render_line_to_text);
		let text_line = text.get(idx).map_or_else(String::new, render_line_to_text);
		let _ = writeln!(&mut body, "{gutter_line}{text_line}");
	}

	body
}

fn render_line_to_text(line: &RenderLine<'_>) -> String {
	line.spans.iter().map(|span| span.content.as_ref()).collect()
}

fn build_surface_summary(
	overlay_kind: Option<OverlayControllerKind>,
	overlay_panes: &[OverlayPaneRenderTarget],
	completion_plan: Option<&CompletionRenderPlan>,
	snippet_plan: Option<&SnippetChoiceRenderPlan>,
	info_popup_plan: &[InfoPopupRenderTarget],
) -> String {
	let mut lines = Vec::new();

	match overlay_kind {
		Some(kind) => {
			lines.push(format!("overlay={kind:?} panes={}", overlay_panes.len()));
			for pane in overlay_panes.iter().take(3) {
				lines.push(format!("  {:?} {}", pane.role, rect_brief(pane.rect)));
			}
			if overlay_panes.len() > 3 {
				lines.push(format!("  ... {} more panes", overlay_panes.len() - 3));
			}
		}
		None => lines.push(String::from("overlay=none")),
	}

	match completion_plan {
		Some(plan) => {
			let selected = plan
				.items
				.iter()
				.find(|item| item.selected)
				.map_or_else(|| String::from("-"), |item| item.label.clone());
			lines.push(format!(
				"completion=visible rows={} selected={} kind_col={} right_col={}",
				plan.items.len(),
				selected,
				plan.show_kind,
				plan.show_right
			));
		}
		None => lines.push(String::from("completion=hidden")),
	}

	match snippet_plan {
		Some(plan) => {
			let selected = plan
				.items
				.iter()
				.find(|item| item.selected)
				.map_or_else(|| String::from("-"), |item| item.option.clone());
			lines.push(format!("snippet_choice=visible rows={} selected={selected}", plan.items.len()));
		}
		None => lines.push(String::from("snippet_choice=hidden")),
	}

	if info_popup_plan.is_empty() {
		lines.push(String::from("info_popups=none"));
	} else {
		lines.push(format!("info_popups={}", info_popup_plan.len()));
		for popup in info_popup_plan.iter().take(2) {
			let anchor = match popup.anchor {
				InfoPopupRenderAnchor::Center => String::from("center"),
				InfoPopupRenderAnchor::Point { x, y } => format!("point@{x},{y}"),
			};
			lines.push(format!("  popup#{} {} {}x{}", popup.id.0, anchor, popup.content_width, popup.content_height));
		}
		if info_popup_plan.len() > 2 {
			lines.push(format!("  ... {} more popups", info_popup_plan.len() - 2));
		}
	}

	lines.join("\n")
}

fn format_completion_preview(plan: Option<&CompletionRenderPlan>) -> String {
	let Some(plan) = plan else {
		return String::from("completion_rows=hidden");
	};

	let mut lines = Vec::new();
	lines.push(format!(
		"completion_rows={} target_width={} kind_col={} right_col={}",
		plan.items.len(),
		plan.target_row_width,
		plan.show_kind,
		plan.show_right
	));

	for item in plan.items.iter().take(8) {
		let marker = if item.selected { ">" } else { " " };
		let mut row = format!("{marker} {}", item.label);
		if plan.show_kind {
			row.push_str(&format!("  [{:?}]", item.kind));
		}
		if plan.show_right
			&& let Some(right) = &item.right
		{
			row.push_str(&format!("  ({right})"));
		}
		lines.push(row);
	}

	if plan.items.len() > 8 {
		lines.push(format!("... {} more completion rows", plan.items.len() - 8));
	}

	lines.join("\n")
}

fn format_snippet_preview(plan: Option<&SnippetChoiceRenderPlan>) -> String {
	let Some(plan) = plan else {
		return String::from("snippet_rows=hidden");
	};

	let mut lines = Vec::new();
	lines.push(format!("snippet_rows={} target_width={}", plan.items.len(), plan.target_row_width));

	for item in plan.items.iter().take(8) {
		let marker = if item.selected { ">" } else { " " };
		lines.push(format!("{marker} {}", item.option));
	}

	if plan.items.len() > 8 {
		lines.push(format!("... {} more snippet rows", plan.items.len() - 8));
	}

	lines.join("\n")
}

fn rect_brief(rect: Rect) -> String {
	format!("{}x{}@{},{}", rect.width, rect.height, rect.x, rect.y)
}

fn clipboard_paste_task(event: &Event) -> Option<Task<Message>> {
	let Event::Keyboard(keyboard::Event::KeyPressed {
		key,
		modified_key,
		physical_key,
		modifiers,
		..
	}) = event
	else {
		return None;
	};

	if !is_paste_shortcut(key, modified_key, *physical_key, *modifiers) {
		return None;
	}

	Some(iced::clipboard::read_text().map(Message::ClipboardRead))
}

fn is_paste_shortcut(key: &keyboard::Key, modified_key: &keyboard::Key, physical_key: keyboard::key::Physical, modifiers: keyboard::Modifiers) -> bool {
	if matches!(key.as_ref(), keyboard::Key::Named(keyboard::key::Named::Paste))
		|| matches!(modified_key.as_ref(), keyboard::Key::Named(keyboard::key::Named::Paste))
	{
		return true;
	}

	if !modifiers.command() {
		return false;
	}

	modified_key.to_latin(physical_key).is_some_and(|ch| ch.eq_ignore_ascii_case(&'v'))
		|| key.to_latin(physical_key).is_some_and(|ch| ch.eq_ignore_ascii_case(&'v'))
}

fn map_event(event: Event, cell_metrics: CellMetrics, event_state: &mut EventBridgeState) -> Option<RuntimeEvent> {
	match event {
		Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
			event_state.modifiers = map_modifiers(modifiers);
			None
		}
		Event::Keyboard(keyboard::Event::KeyPressed {
			modified_key,
			physical_key,
			modifiers,
			..
		}) => {
			event_state.modifiers = map_modifiers(modifiers);
			map_key_event(modified_key, physical_key, modifiers).map(RuntimeEvent::Key)
		}
		Event::Keyboard(keyboard::Event::KeyReleased { modifiers, .. }) => {
			event_state.modifiers = map_modifiers(modifiers);
			None
		}
		Event::Mouse(mouse::Event::CursorMoved { position }) => {
			let col = logical_pixels_to_cell_index(position.x, cell_metrics.width_px);
			let row = logical_pixels_to_cell_index(position.y, cell_metrics.height_px);
			event_state.mouse_col = col;
			event_state.mouse_row = row;

			Some(RuntimeEvent::Mouse(match event_state.mouse_button {
				Some(button) => CoreMouseEvent::Drag {
					button,
					row,
					col,
					modifiers: event_state.modifiers,
				},
				None => CoreMouseEvent::Move { row, col },
			}))
		}
		Event::Mouse(mouse::Event::ButtonPressed(button)) => {
			let button = map_mouse_button(button)?;
			event_state.mouse_button = Some(button);

			Some(RuntimeEvent::Mouse(CoreMouseEvent::Press {
				button,
				row: event_state.mouse_row,
				col: event_state.mouse_col,
				modifiers: event_state.modifiers,
			}))
		}
		Event::Mouse(mouse::Event::ButtonReleased(_)) => {
			event_state.mouse_button = None;
			Some(RuntimeEvent::Mouse(CoreMouseEvent::Release {
				row: event_state.mouse_row,
				col: event_state.mouse_col,
			}))
		}
		Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
			let direction = map_scroll_delta(delta)?;
			Some(RuntimeEvent::Mouse(CoreMouseEvent::Scroll {
				direction,
				row: event_state.mouse_row,
				col: event_state.mouse_col,
				modifiers: event_state.modifiers,
			}))
		}
		Event::Window(window::Event::Opened { size, .. }) | Event::Window(window::Event::Resized(size)) => {
			let (cols, rows) = cell_metrics.to_grid(size.width, size.height);
			Some(RuntimeEvent::WindowResized { cols, rows })
		}
		Event::InputMethod(event) => map_input_method_event(event, event_state),
		Event::Window(window::Event::Focused) => Some(RuntimeEvent::FocusIn),
		Event::Window(window::Event::Unfocused) => Some(RuntimeEvent::FocusOut),
		_ => None,
	}
}

fn map_input_method_event(event: input_method::Event, event_state: &mut EventBridgeState) -> Option<RuntimeEvent> {
	match event {
		input_method::Event::Opened | input_method::Event::Closed => {
			event_state.ime_preedit = None;
			None
		}
		input_method::Event::Preedit(text, _selection) => {
			event_state.ime_preedit = if text.is_empty() { None } else { Some(text) };
			None
		}
		input_method::Event::Commit(text) if !text.is_empty() => {
			event_state.ime_preedit = None;
			Some(RuntimeEvent::Paste(text))
		}
		input_method::Event::Commit(_) => {
			event_state.ime_preedit = None;
			None
		}
	}
}

fn ime_preedit_label(preedit: Option<&str>) -> String {
	let Some(preedit) = preedit else {
		return String::from("-");
	};

	const MAX_CHARS: usize = 24;
	let total = preedit.chars().count();
	if total <= MAX_CHARS {
		return preedit.to_string();
	}

	let prefix: String = preedit.chars().take(MAX_CHARS).collect();
	format!("{prefix}...")
}

fn logical_pixels_to_cells(logical_px: f32, cell_px: f32) -> u16 {
	if !logical_px.is_finite() || !cell_px.is_finite() || cell_px <= 0.0 {
		return 1;
	}

	let cells = (logical_px / cell_px).floor();
	cells.clamp(1.0, u16::MAX as f32) as u16
}

fn logical_pixels_to_cell_index(logical_px: f32, cell_px: f32) -> u16 {
	logical_pixels_to_cells(logical_px, cell_px).saturating_sub(1)
}

fn map_modifiers(modifiers: keyboard::Modifiers) -> Modifiers {
	Modifiers {
		ctrl: modifiers.control(),
		alt: modifiers.alt(),
		shift: modifiers.shift(),
	}
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
	let modifiers = map_modifiers(modifiers);

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

fn map_mouse_button(button: mouse::Button) -> Option<CoreMouseButton> {
	match button {
		mouse::Button::Left => Some(CoreMouseButton::Left),
		mouse::Button::Right => Some(CoreMouseButton::Right),
		mouse::Button::Middle => Some(CoreMouseButton::Middle),
		mouse::Button::Back | mouse::Button::Forward | mouse::Button::Other(_) => None,
	}
}

fn map_scroll_delta(delta: mouse::ScrollDelta) -> Option<ScrollDirection> {
	let (x, y) = match delta {
		mouse::ScrollDelta::Lines { x, y } | mouse::ScrollDelta::Pixels { x, y } => (x, y),
	};

	if y.abs() >= x.abs() && y != 0.0 {
		return Some(if y > 0.0 { ScrollDirection::Up } else { ScrollDirection::Down });
	}

	if x != 0.0 {
		return Some(if x > 0.0 { ScrollDirection::Right } else { ScrollDirection::Left });
	}

	None
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
	fn logical_pixels_to_cell_index_is_zero_based() {
		assert_eq!(logical_pixels_to_cell_index(0.0, 8.0), 0);
		assert_eq!(logical_pixels_to_cell_index(7.9, 8.0), 0);
		assert_eq!(logical_pixels_to_cell_index(8.0, 8.0), 0);
		assert_eq!(logical_pixels_to_cell_index(16.0, 8.0), 1);
	}

	#[test]
	fn parse_cell_size_falls_back_for_invalid_values() {
		assert_eq!(parse_cell_size(Some(String::from("abc")), 8.0), 8.0);
		assert_eq!(parse_cell_size(Some(String::from("0")), 8.0), 8.0);
		assert_eq!(parse_cell_size(Some(String::from("-4")), 8.0), 8.0);
		assert_eq!(parse_cell_size(None, 8.0), 8.0);
	}

	#[test]
	fn map_scroll_delta_prefers_vertical_direction() {
		assert_eq!(map_scroll_delta(mouse::ScrollDelta::Lines { x: 1.0, y: -2.0 }), Some(ScrollDirection::Down));
		assert_eq!(map_scroll_delta(mouse::ScrollDelta::Pixels { x: -2.0, y: 1.0 }), Some(ScrollDirection::Left));
		assert_eq!(map_scroll_delta(mouse::ScrollDelta::Lines { x: 0.0, y: 0.0 }), None);
	}

	#[test]
	fn is_paste_shortcut_matches_command_v() {
		let key = keyboard::Key::Character("v".into());
		let physical = keyboard::key::Physical::Code(keyboard::key::Code::KeyV);
		assert!(is_paste_shortcut(&key, &key, physical, keyboard::Modifiers::COMMAND));
	}

	#[test]
	fn is_paste_shortcut_matches_named_paste_key() {
		let key = keyboard::Key::Named(keyboard::key::Named::Paste);
		let physical = keyboard::key::Physical::Code(keyboard::key::Code::Paste);
		assert!(is_paste_shortcut(&key, &key, physical, keyboard::Modifiers::default()));
	}

	#[test]
	fn map_input_method_event_maps_commit_to_paste() {
		let mut state = EventBridgeState::default();
		assert_eq!(
			map_input_method_event(input_method::Event::Commit(String::from("hello")), &mut state),
			Some(RuntimeEvent::Paste(String::from("hello")))
		);
		assert_eq!(map_input_method_event(input_method::Event::Commit(String::new()), &mut state), None);
		assert_eq!(map_input_method_event(input_method::Event::Opened, &mut state), None);
	}

	#[test]
	fn map_input_method_event_tracks_preedit_state() {
		let mut state = EventBridgeState::default();
		assert_eq!(
			map_input_method_event(input_method::Event::Preedit(String::from("compose"), None), &mut state),
			None
		);
		assert_eq!(state.ime_preedit.as_deref(), Some("compose"));

		assert_eq!(
			map_input_method_event(input_method::Event::Commit(String::from("x")), &mut state),
			Some(RuntimeEvent::Paste(String::from("x")))
		);
		assert_eq!(state.ime_preedit, None);
	}

	#[test]
	fn ime_preedit_label_truncates_long_content() {
		assert_eq!(ime_preedit_label(None), "-");
		assert_eq!(ime_preedit_label(Some("short")), "short");
		assert_eq!(ime_preedit_label(Some("abcdefghijklmnopqrstuvwxyz")), "abcdefghijklmnopqrstuvwx...");
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
