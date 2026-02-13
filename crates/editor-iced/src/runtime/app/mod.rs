use iced::widget::scrollable::{Direction as ScrollDirection, Scrollbar};
use iced::widget::text::Wrapping;
use iced::widget::{Column, column, container, rich_text, row, rule, scrollable, span, text};
use iced::{Background, Color, Element, Event, Fill, Font, Subscription, Task, border, event, font, keyboard, time, window};
use xeno_editor::Editor;
use xeno_editor::completion::{CompletionRenderItem, CompletionRenderPlan};
use xeno_editor::geometry::Rect;
use xeno_editor::info_popup::InfoPopupRenderAnchor;
use xeno_editor::render_api::RenderLine;
use xeno_editor::runtime::{CursorStyle, LoopDirective, RuntimeEvent};
use xeno_editor::snippet::{SnippetChoiceRenderItem, SnippetChoiceRenderPlan};
use xeno_editor::ui::StatuslineRenderStyle;
use xeno_primitives::{Color as UiColor, Style as UiStyle};

use super::{DEFAULT_POLL_INTERVAL, EventBridgeState, Snapshot, StartupOptions, SurfaceSnapshot, build_snapshot, configure_linux_backend, map_event};

const DEFAULT_INSPECTOR_WIDTH_PX: f32 = 320.0;
const MIN_INSPECTOR_WIDTH_PX: f32 = 160.0;

#[derive(Debug, Clone)]
pub(crate) enum Message {
	Tick(time::Instant),
	Event(Event),
	ClipboardRead(Result<std::sync::Arc<String>, iced::clipboard::Error>),
}

pub(crate) struct IcedEditorApp {
	runtime: tokio::runtime::Runtime,
	editor: Editor,
	directive: LoopDirective,
	quit_hook_emitted: bool,
	snapshot: Snapshot,
	cell_metrics: super::CellMetrics,
	event_state: EventBridgeState,
	layout: LayoutConfig,
}

#[derive(Debug, Clone, Copy)]
struct LayoutConfig {
	inspector_width_px: f32,
	show_inspector: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorTone {
	Normal,
	Meta,
	Selected,
}

impl LayoutConfig {
	fn from_env() -> Self {
		let inspector_width_px = parse_inspector_width(std::env::var("XENO_ICED_INSPECTOR_WIDTH_PX").ok().as_deref());
		let show_inspector = parse_show_inspector(std::env::var("XENO_ICED_SHOW_INSPECTOR").ok().as_deref());

		Self {
			inspector_width_px,
			show_inspector,
		}
	}
}

fn parse_inspector_width(value: Option<&str>) -> f32 {
	value
		.and_then(|value| value.parse::<f32>().ok())
		.filter(|width| width.is_finite() && *width >= MIN_INSPECTOR_WIDTH_PX)
		.unwrap_or(DEFAULT_INSPECTOR_WIDTH_PX)
}

fn parse_show_inspector(value: Option<&str>) -> bool {
	let Some(value) = value else {
		return true;
	};

	!matches!(value.trim().to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off")
}

impl IcedEditorApp {
	pub(crate) fn boot(startup: StartupOptions) -> (Self, Task<Message>) {
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
			cell_metrics: super::CellMetrics::from_env(),
			event_state: EventBridgeState::default(),
			layout: LayoutConfig::from_env(),
		};

		app.directive = app.runtime.block_on(app.editor.pump());
		app.rebuild_snapshot();

		(app, Task::none())
	}

	pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
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

	pub(crate) fn view(&self) -> Element<'_, Message> {
		let header_block = text(&self.snapshot.header).font(Font::MONOSPACE);

		let mut document_rows = column![].spacing(0);
		for line in &self.snapshot.document_lines {
			document_rows = document_rows.push(render_document_line(line));
		}
		let document_scroll = scrollable(document_rows)
			.direction(ScrollDirection::Vertical(Scrollbar::hidden()))
			.height(Fill)
			.width(Fill);
		let document = container(document_scroll).width(Fill).height(Fill).clip(true);
		let inspector_rows = render_inspector_rows(&self.snapshot.surface);

		let inspector_scroll = scrollable(inspector_rows)
			.direction(ScrollDirection::Vertical(Scrollbar::hidden()))
			.height(Fill)
			.width(Fill);
		let inspector = container(inspector_scroll).width(self.layout.inspector_width_px).height(Fill).clip(true);

		let panes = if self.layout.show_inspector {
			row![document, rule::vertical(1), inspector].spacing(8).height(Fill)
		} else {
			row![document].height(Fill)
		};
		let statusline = render_statusline(&self.editor, &self.snapshot.statusline_segments);

		let content = column![header_block, panes, statusline].spacing(8).padding(12).width(Fill).height(Fill);

		container(content).width(Fill).height(Fill).clip(true).into()
	}

	pub(crate) fn subscription(&self) -> Subscription<Message> {
		let mut tick_interval = self.directive.poll_timeout.unwrap_or(DEFAULT_POLL_INTERVAL);
		if tick_interval.is_zero() {
			tick_interval = DEFAULT_POLL_INTERVAL;
		}

		Subscription::batch([event::listen().map(Message::Event), time::every(tick_interval).map(Message::Tick)])
	}

	pub(crate) fn title(&self) -> String {
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
		self.snapshot = build_snapshot(&mut self.editor, self.event_state.ime_preedit());
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

fn render_inspector_rows(surface: &SurfaceSnapshot) -> Column<'static, Message> {
	let mut rows = column![].spacing(2);
	rows = push_inspector_section_title(rows, "surface");
	rows = append_surface_rows(rows, surface);
	rows = rows.push(styled_inspector_text(String::new(), InspectorTone::Normal));
	rows = push_inspector_section_title(rows, "completion");
	rows = append_completion_rows(rows, surface.completion_plan.as_ref());
	rows = rows.push(styled_inspector_text(String::new(), InspectorTone::Normal));
	rows = push_inspector_section_title(rows, "snippet");
	append_snippet_rows(rows, surface.snippet_plan.as_ref())
}

fn push_inspector_section_title(mut rows: Column<'static, Message>, title: &str) -> Column<'static, Message> {
	rows = rows.push(styled_inspector_text(format!("{title}:"), InspectorTone::Normal));
	rows
}

fn styled_inspector_text(content: impl Into<String>, tone: InspectorTone) -> iced::widget::Text<'static> {
	let mut row_text = text(content.into()).font(Font::MONOSPACE).wrapping(Wrapping::None);
	row_text = match tone {
		InspectorTone::Normal => row_text,
		InspectorTone::Meta => row_text.color(Color::from_rgb8(0x6A, 0x73, 0x7D)),
		InspectorTone::Selected => row_text.color(Color::from_rgb8(0x0B, 0x72, 0x2B)),
	};
	row_text
}

fn append_surface_rows(mut rows: Column<'static, Message>, surface: &SurfaceSnapshot) -> Column<'static, Message> {
	match surface.overlay_kind {
		Some(kind) => {
			rows = rows.push(styled_inspector_text(
				format!("overlay={kind:?} panes={}", surface.overlay_panes.len()),
				InspectorTone::Meta,
			));
			for pane in surface.overlay_panes.iter().take(3) {
				rows = rows.push(styled_inspector_text(
					format!("  {:?} {}", pane.role, rect_brief(pane.rect)),
					InspectorTone::Meta,
				));
			}
			if surface.overlay_panes.len() > 3 {
				rows = rows.push(styled_inspector_text(
					format!("  ... {} more panes", surface.overlay_panes.len() - 3),
					InspectorTone::Meta,
				));
			}
		}
		None => {
			rows = rows.push(styled_inspector_text("overlay=none", InspectorTone::Meta));
		}
	}

	match surface.completion_plan.as_ref() {
		Some(plan) => {
			let selected = plan
				.items
				.iter()
				.find(|item| item.selected)
				.map_or_else(|| String::from("-"), |item| item.label.clone());
			rows = rows.push(styled_inspector_text(
				format!(
					"completion=visible rows={} selected={} kind_col={} right_col={}",
					plan.items.len(),
					selected,
					plan.show_kind,
					plan.show_right
				),
				InspectorTone::Meta,
			));
		}
		None => {
			rows = rows.push(styled_inspector_text("completion=hidden", InspectorTone::Meta));
		}
	}

	match surface.snippet_plan.as_ref() {
		Some(plan) => {
			let selected = plan
				.items
				.iter()
				.find(|item| item.selected)
				.map_or_else(|| String::from("-"), |item| item.option.clone());
			rows = rows.push(styled_inspector_text(
				format!("snippet_choice=visible rows={} selected={selected}", plan.items.len()),
				InspectorTone::Meta,
			));
		}
		None => {
			rows = rows.push(styled_inspector_text("snippet_choice=hidden", InspectorTone::Meta));
		}
	}

	if surface.info_popup_plan.is_empty() {
		rows = rows.push(styled_inspector_text("info_popups=none", InspectorTone::Meta));
	} else {
		rows = rows.push(styled_inspector_text(
			format!("info_popups={}", surface.info_popup_plan.len()),
			InspectorTone::Meta,
		));
		for popup in surface.info_popup_plan.iter().take(2) {
			let anchor = match popup.anchor {
				InfoPopupRenderAnchor::Center => String::from("center"),
				InfoPopupRenderAnchor::Point { x, y } => format!("point@{x},{y}"),
			};
			rows = rows.push(styled_inspector_text(
				format!("  popup#{} {} {}x{}", popup.id.0, anchor, popup.content_width, popup.content_height),
				InspectorTone::Meta,
			));
		}
		if surface.info_popup_plan.len() > 2 {
			rows = rows.push(styled_inspector_text(
				format!("  ... {} more popups", surface.info_popup_plan.len() - 2),
				InspectorTone::Meta,
			));
		}
	}

	rows
}

fn append_completion_rows(mut rows: Column<'static, Message>, plan: Option<&CompletionRenderPlan>) -> Column<'static, Message> {
	let Some(plan) = plan else {
		rows = rows.push(styled_inspector_text("completion_rows=hidden", InspectorTone::Meta));
		return rows;
	};

	rows = rows.push(styled_inspector_text(
		format!(
			"completion_rows={} target_width={} kind_col={} right_col={}",
			plan.items.len(),
			plan.target_row_width,
			plan.show_kind,
			plan.show_right
		),
		InspectorTone::Meta,
	));

	for item in plan.items.iter().take(8) {
		let tone = if item.selected { InspectorTone::Selected } else { InspectorTone::Normal };
		rows = rows.push(styled_inspector_text(completion_row_label(plan, item), tone));
	}

	if plan.items.len() > 8 {
		rows = rows.push(styled_inspector_text(
			format!("... {} more completion rows", plan.items.len() - 8),
			InspectorTone::Meta,
		));
	}

	rows
}

fn completion_row_label(plan: &CompletionRenderPlan, item: &CompletionRenderItem) -> String {
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
	row
}

fn append_snippet_rows(mut rows: Column<'static, Message>, plan: Option<&SnippetChoiceRenderPlan>) -> Column<'static, Message> {
	let Some(plan) = plan else {
		rows = rows.push(styled_inspector_text("snippet_rows=hidden", InspectorTone::Meta));
		return rows;
	};

	rows = rows.push(styled_inspector_text(
		format!("snippet_rows={} target_width={}", plan.items.len(), plan.target_row_width),
		InspectorTone::Meta,
	));

	for item in plan.items.iter().take(8) {
		let tone = if item.selected { InspectorTone::Selected } else { InspectorTone::Normal };
		rows = rows.push(styled_inspector_text(snippet_row_label(item), tone));
	}

	if plan.items.len() > 8 {
		rows = rows.push(styled_inspector_text(
			format!("... {} more snippet rows", plan.items.len() - 8),
			InspectorTone::Meta,
		));
	}

	rows
}

fn snippet_row_label(item: &SnippetChoiceRenderItem) -> String {
	let marker = if item.selected { ">" } else { " " };
	format!("{marker} {}", item.option)
}

fn rect_brief(rect: Rect) -> String {
	format!("{}x{}@{},{}", rect.width, rect.height, rect.x, rect.y)
}

fn render_document_line(line: &RenderLine<'_>) -> Element<'static, Message> {
	let mut spans = Vec::new();
	let line_color = line.style.and_then(style_fg_to_iced);

	for render_span in &line.spans {
		let mut segment = span::<(), _>(render_span.content.as_ref().to_string());
		if let Some(color) = style_fg_to_iced(render_span.style).or(line_color) {
			segment = segment.color(color);
		}
		spans.push(segment);
	}

	if spans.is_empty() {
		spans.push(span::<(), _>(String::new()));
	}

	rich_text(spans).font(Font::MONOSPACE).wrapping(Wrapping::None).into()
}

fn render_statusline(editor: &Editor, segments: &[xeno_editor::ui::StatuslineRenderSegment]) -> Element<'static, Message> {
	let mut spans = Vec::new();

	for segment in segments {
		let mut item = span::<(), _>(segment.text.clone()).font(Font::MONOSPACE);
		let style = editor.statusline_segment_style(segment.style);
		if let Some(color) = style_fg_to_iced(style) {
			item = item.color(color);
		}
		if let Some(bg) = style_bg_to_iced(style) {
			item = item.background(Background::Color(bg)).border(border::rounded(0));
		}
		if matches!(segment.style, StatuslineRenderStyle::Mode) {
			item = item.font(Font {
				weight: font::Weight::Bold,
				..Font::MONOSPACE
			});
		}
		spans.push(item);
	}

	if spans.is_empty() {
		spans.push(span::<(), _>(String::new()).font(Font::MONOSPACE));
	}

	rich_text(spans).wrapping(Wrapping::None).into()
}

fn style_fg_to_iced(style: UiStyle) -> Option<Color> {
	style.fg.and_then(map_ui_color)
}

fn style_bg_to_iced(style: UiStyle) -> Option<Color> {
	style.bg.and_then(map_ui_color)
}

fn map_ui_color(color: UiColor) -> Option<Color> {
	match color {
		UiColor::Reset => None,
		UiColor::Black => Some(Color::from_rgb8(0x00, 0x00, 0x00)),
		UiColor::Red => Some(Color::from_rgb8(0x80, 0x00, 0x00)),
		UiColor::Green => Some(Color::from_rgb8(0x00, 0x80, 0x00)),
		UiColor::Yellow => Some(Color::from_rgb8(0x80, 0x80, 0x00)),
		UiColor::Blue => Some(Color::from_rgb8(0x00, 0x00, 0x80)),
		UiColor::Magenta => Some(Color::from_rgb8(0x80, 0x00, 0x80)),
		UiColor::Cyan => Some(Color::from_rgb8(0x00, 0x80, 0x80)),
		UiColor::Gray => Some(Color::from_rgb8(0xC0, 0xC0, 0xC0)),
		UiColor::DarkGray => Some(Color::from_rgb8(0x80, 0x80, 0x80)),
		UiColor::LightRed => Some(Color::from_rgb8(0xFF, 0x00, 0x00)),
		UiColor::LightGreen => Some(Color::from_rgb8(0x00, 0xFF, 0x00)),
		UiColor::LightYellow => Some(Color::from_rgb8(0xFF, 0xFF, 0x00)),
		UiColor::LightBlue => Some(Color::from_rgb8(0x00, 0x00, 0xFF)),
		UiColor::LightMagenta => Some(Color::from_rgb8(0xFF, 0x00, 0xFF)),
		UiColor::LightCyan => Some(Color::from_rgb8(0x00, 0xFF, 0xFF)),
		UiColor::White => Some(Color::from_rgb8(0xFF, 0xFF, 0xFF)),
		UiColor::Rgb(r, g, b) => Some(Color::from_rgb8(r, g, b)),
		UiColor::Indexed(index) => Some(map_indexed_color(index)),
	}
}

fn map_indexed_color(index: u8) -> Color {
	const BASE: [(u8, u8, u8); 16] = [
		(0x00, 0x00, 0x00),
		(0x80, 0x00, 0x00),
		(0x00, 0x80, 0x00),
		(0x80, 0x80, 0x00),
		(0x00, 0x00, 0x80),
		(0x80, 0x00, 0x80),
		(0x00, 0x80, 0x80),
		(0xC0, 0xC0, 0xC0),
		(0x80, 0x80, 0x80),
		(0xFF, 0x00, 0x00),
		(0x00, 0xFF, 0x00),
		(0xFF, 0xFF, 0x00),
		(0x00, 0x00, 0xFF),
		(0xFF, 0x00, 0xFF),
		(0x00, 0xFF, 0xFF),
		(0xFF, 0xFF, 0xFF),
	];
	const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

	if index < 16 {
		let (r, g, b) = BASE[index as usize];
		return Color::from_rgb8(r, g, b);
	}

	if (16..=231).contains(&index) {
		let value = index - 16;
		let r = CUBE[(value / 36) as usize];
		let g = CUBE[((value % 36) / 6) as usize];
		let b = CUBE[(value % 6) as usize];
		return Color::from_rgb8(r, g, b);
	}

	let gray = 8u8.saturating_add((index - 232) * 10);
	Color::from_rgb8(gray, gray, gray)
}

pub fn run(startup: StartupOptions) -> iced::Result {
	configure_linux_backend();

	iced::application(move || IcedEditorApp::boot(startup.clone()), IcedEditorApp::update, IcedEditorApp::view)
		.title(IcedEditorApp::title)
		.subscription(IcedEditorApp::subscription)
		.window_size((1200.0, 800.0))
		.run()
}

#[cfg(test)]
mod tests;
