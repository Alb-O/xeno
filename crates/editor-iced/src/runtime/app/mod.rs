mod inspector;
mod render;

use iced::widget::scrollable::{Direction as ScrollDirection, Scrollbar};
use iced::widget::{column, container, mouse_area, pin, row, rule, scrollable, sensor, stack, text};
use iced::{Element, Event, Fill, Font, Pixels, Point, Size, Subscription, Task, event, keyboard, mouse, time, window};
use xeno_editor::Editor;
use xeno_editor::render_api::{CompletionRenderPlan, Rect as CoreRect};
use xeno_editor::runtime::{CursorStyle, LoopDirective, RuntimeEvent};

use self::inspector::render_inspector_rows;
use self::render::{background_style, render_palette_completion_menu, render_render_lines, render_statusline};
use super::{DEFAULT_POLL_INTERVAL, EventBridgeState, HeaderSnapshot, Snapshot, StartupOptions, build_snapshot, configure_linux_backend, map_event};

const DEFAULT_INSPECTOR_WIDTH_PX: f32 = 320.0;
const MIN_INSPECTOR_WIDTH_PX: f32 = 160.0;

#[derive(Debug, Clone)]
pub(crate) enum Message {
	Tick(time::Instant),
	Event(Event),
	ClipboardRead(Result<std::sync::Arc<String>, iced::clipboard::Error>),
	DocumentViewportChanged(Size),
	DocumentCursorMoved(Point),
	DocumentButtonPressed(mouse::Button),
	DocumentButtonReleased(mouse::Button),
	DocumentScrolled(mouse::ScrollDelta),
}

pub(crate) struct IcedEditorApp {
	runtime: tokio::runtime::Runtime,
	editor: Editor,
	directive: LoopDirective,
	quit_hook_emitted: bool,
	snapshot: Snapshot,
	cell_metrics: super::CellMetrics,
	event_state: EventBridgeState,
	document_viewport_cells: Option<(u16, u16)>,
	coordinate_scale: CoordinateScale,
	layout: LayoutConfig,
}

#[derive(Debug, Clone, Copy)]
struct LayoutConfig {
	inspector_width_px: f32,
	show_inspector: bool,
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

#[derive(Debug, Clone, Copy)]
struct CoordinateScale {
	x: f32,
	y: f32,
}

#[derive(Debug, Clone)]
struct PaletteCompletionOverlay {
	x_px: f32,
	y_px: f32,
	width_px: f32,
	plan: CompletionRenderPlan,
}

impl CoordinateScale {
	fn from_env() -> Self {
		let uniform = parse_coordinate_scale(std::env::var("XENO_ICED_COORD_SCALE").ok().as_deref()).unwrap_or(1.0);
		let x = parse_coordinate_scale(std::env::var("XENO_ICED_COORD_SCALE_X").ok().as_deref()).unwrap_or(uniform);
		let y = parse_coordinate_scale(std::env::var("XENO_ICED_COORD_SCALE_Y").ok().as_deref()).unwrap_or(uniform);
		Self { x, y }
	}

	fn normalize_point(self, point: Point) -> Point {
		Point::new(point.x / self.x, point.y / self.y)
	}

	fn normalize_size(self, size: Size) -> Size {
		Size::new(size.width / self.x, size.height / self.y)
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

fn parse_coordinate_scale(value: Option<&str>) -> Option<f32> {
	let value = value?;
	let scale = value.parse::<f32>().ok()?;
	(scale.is_finite() && scale > 0.0).then_some(scale)
}

fn format_header_line(header: &HeaderSnapshot) -> String {
	format!(
		"mode={} cursor={}:{} buffers={} ime_preedit={}",
		header.mode, header.cursor_line, header.cursor_col, header.buffers, header.ime_preedit
	)
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
			document_viewport_cells: None,
			coordinate_scale: CoordinateScale::from_env(),
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
			Message::DocumentViewportChanged(document_size) => {
				let document_size = self.coordinate_scale.normalize_size(document_size);
				self.apply_document_viewport_size(document_size);
			}
			Message::DocumentCursorMoved(position) => {
				let position = self.coordinate_scale.normalize_point(position);
				self.forward_document_mouse_event(mouse::Event::CursorMoved { position });
			}
			Message::DocumentButtonPressed(button) => {
				self.forward_document_mouse_event(mouse::Event::ButtonPressed(button));
			}
			Message::DocumentButtonReleased(button) => {
				self.forward_document_mouse_event(mouse::Event::ButtonReleased(button));
			}
			Message::DocumentScrolled(delta) => {
				self.forward_document_mouse_event(mouse::Event::WheelScrolled { delta });
			}
			Message::Event(event) => {
				if matches!(event, Event::Window(window::Event::CloseRequested)) {
					self.directive.should_quit = true;
				} else if let Some(task) = clipboard_paste_task(&event) {
					return task;
				} else if matches!(event, Event::Mouse(_)) {
				} else if matches!(event, Event::Window(window::Event::Opened { .. }) | Event::Window(window::Event::Resized(_))) {
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
		let ui_bg = self.editor.config().theme.colors.ui.bg;
		let popup_bg = self.editor.config().theme.colors.popup.bg;
		let line_height_px = self.cell_metrics.height_px();
		let header_block = text(format_header_line(&self.snapshot.header)).font(Font::MONOSPACE);

		// Base background.
		let document_bg = container(text("")).height(Fill).width(Fill).style(move |_theme| background_style(ui_bg));
		let mut scene_layers: Vec<Element<'_, Message>> = vec![document_bg.into()];

		// Document views (all split panes).
		for view_plan in &self.snapshot.document_views {
			if view_plan.gutter_rect().width > 0 {
				let (gx, gy, gw, gh) = self.rect_px(view_plan.gutter_rect());
				let gutter_widget = container(render_render_lines(view_plan.gutter(), line_height_px))
					.width(gw)
					.height(gh)
					.clip(true);
				scene_layers.push(pin(gutter_widget).x(gx).y(gy).width(Fill).height(Fill).into());
			}

			let (tx, ty, tw, th) = self.rect_px(view_plan.text_rect());
			let text_widget = container(render_render_lines(view_plan.text(), line_height_px)).width(tw).height(th).clip(true);
			scene_layers.push(pin(text_widget).x(tx).y(ty).width(Fill).height(Fill).into());
		}

		// Separators (styled by state + priority).
		let sep_colors = self.separator_theme_colors();
		for sep in &self.snapshot.separators {
			let sep_rect = sep.rect();
			let (sx, sy, sw, sh) = self.rect_px(sep_rect);
			let (fg, bg) = separator_fg_bg(sep.state(), sep.priority(), &sep_colors);
			let sep_text = match sep.direction() {
				xeno_editor::render_api::SplitDirection::Horizontal => "\u{2502}\n".repeat(sep_rect.height as usize),
				xeno_editor::render_api::SplitDirection::Vertical => "\u{2500}".repeat(sep_rect.width as usize),
			};
			let sep_widget = container(
				text(sep_text)
					.font(Font::MONOSPACE)
					.size(Pixels(line_height_px))
					.wrapping(iced::widget::text::Wrapping::None)
					.color(fg),
			)
			.width(sw)
			.height(sh)
			.clip(true)
			.style(move |_theme| iced::widget::container::Style {
				background: Some(iced::Background::Color(bg)),
				..Default::default()
			});
			scene_layers.push(pin(sep_widget).x(sx).y(sy).width(Fill).height(Fill).into());
		}

		// Junction glyphs (styled, from core).
		let cw = self.cell_metrics.width_px();
		let ch = self.cell_metrics.height_px();
		for junc in &self.snapshot.junctions {
			let (fg, bg) = separator_fg_bg(junc.state(), junc.priority(), &sep_colors);
			let jx = f32::from(junc.x()) * cw;
			let jy = f32::from(junc.y()) * ch;
			let junc_widget = container(text(junc.glyph().to_string()).font(Font::MONOSPACE).size(Pixels(line_height_px)).color(fg))
				.width(cw)
				.height(ch)
				.clip(true)
				.style(move |_theme| iced::widget::container::Style {
					background: Some(iced::Background::Color(bg)),
					..Default::default()
				});
			scene_layers.push(pin(junc_widget).x(jx).y(jy).width(Fill).height(Fill).into());
		}

		// Overlay panes (input/list/preview): background at outer rect, content at content_rect.
		for pane_view in &self.snapshot.surface.overlay_pane_views {
			let (bg_x, bg_y, bg_w, bg_h) = self.rect_px(pane_view.rect);
			let bg_widget = container(text("")).width(bg_w).height(bg_h).style(move |_theme| background_style(popup_bg));
			scene_layers.push(pin(bg_widget).x(bg_x).y(bg_y).width(Fill).height(Fill).into());

			if pane_view.gutter_rect.width > 0 {
				let (gx, gy, gw, gh) = self.rect_px(pane_view.gutter_rect);
				let gutter_widget = container(render_render_lines(&pane_view.gutter, line_height_px))
					.width(gw)
					.height(gh)
					.clip(true);
				scene_layers.push(pin(gutter_widget).x(gx).y(gy).width(Fill).height(Fill).into());
			}

			let (tx, ty, tw, th) = self.rect_px(pane_view.text_rect);
			let text_widget = container(render_render_lines(&pane_view.text, line_height_px)).width(tw).height(th).clip(true);
			scene_layers.push(pin(text_widget).x(tx).y(ty).width(Fill).height(Fill).into());
		}

		// Overlay completion menu.
		if let Some(menu) = self.palette_completion_overlay() {
			let menu_widget = container(render_palette_completion_menu(&self.editor, &menu.plan, line_height_px))
				.width(menu.width_px)
				.clip(true);
			scene_layers.push(pin(menu_widget).x(menu.x_px).y(menu.y_px).width(Fill).height(Fill).into());
		}

		// Info popups: background at outer rect, content at inner_rect.
		for popup_view in &self.snapshot.surface.info_popup_views {
			let (bg_x, bg_y, bg_w, bg_h) = self.rect_px(popup_view.rect);
			let bg_widget = container(text("")).width(bg_w).height(bg_h).style(move |_theme| background_style(popup_bg));
			scene_layers.push(pin(bg_widget).x(bg_x).y(bg_y).width(Fill).height(Fill).into());

			let (tx, ty, tw, th) = self.rect_px(popup_view.inner_rect);
			let text_widget = container(render_render_lines(&popup_view.text, line_height_px)).width(tw).height(th).clip(true);
			scene_layers.push(pin(text_widget).x(tx).y(ty).width(Fill).height(Fill).into());
		}

		let document_scene: Element<'_, Message> = if scene_layers.len() == 1 {
			scene_layers.pop().unwrap()
		} else {
			let mut scene = stack![].width(Fill).height(Fill).clip(true);
			for layer in scene_layers {
				scene = scene.push(layer);
			}
			scene.into()
		};
		let document = mouse_area(
			sensor(document_scene)
				.on_show(Message::DocumentViewportChanged)
				.on_resize(Message::DocumentViewportChanged),
		)
		.on_move(Message::DocumentCursorMoved)
		.on_press(Message::DocumentButtonPressed(mouse::Button::Left))
		.on_release(Message::DocumentButtonReleased(mouse::Button::Left))
		.on_right_press(Message::DocumentButtonPressed(mouse::Button::Right))
		.on_right_release(Message::DocumentButtonReleased(mouse::Button::Right))
		.on_middle_press(Message::DocumentButtonPressed(mouse::Button::Middle))
		.on_middle_release(Message::DocumentButtonReleased(mouse::Button::Middle))
		.on_scroll(Message::DocumentScrolled);
		let inspector_rows = render_inspector_rows(&self.snapshot.surface);

		let inspector_scroll = scrollable(inspector_rows)
			.direction(ScrollDirection::Vertical(Scrollbar::hidden()))
			.height(Fill)
			.width(Fill);
		let inspector = container(inspector_scroll)
			.width(self.layout.inspector_width_px)
			.height(Fill)
			.clip(true)
			.style(move |_theme| background_style(popup_bg));

		let panes = if self.layout.show_inspector {
			row![document, rule::vertical(1), inspector].spacing(8).height(Fill)
		} else {
			row![document].height(Fill)
		};
		let statusline = render_statusline(&self.editor, &self.snapshot.statusline_segments, line_height_px);

		let content = column![header_block, panes, statusline].spacing(8).padding(12).width(Fill).height(Fill);

		container(content)
			.width(Fill)
			.height(Fill)
			.clip(true)
			.style(move |_theme| background_style(ui_bg))
			.into()
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
		let doc_bounds = self.document_viewport_cells.map(|(cols, rows)| {
			let doc_rows = rows.saturating_sub(self.editor.statusline_rows());
			CoreRect::new(0, 0, cols, doc_rows)
		});
		self.snapshot = build_snapshot(&mut self.editor, self.event_state.ime_preedit(), doc_bounds);
		self.editor.mark_frame_drawn();
	}

	fn apply_document_viewport_size(&mut self, document_size: Size) {
		let (cols, rows) = viewport_grid_from_document_size(&self.editor, self.cell_metrics, document_size);
		if self.document_viewport_cells == Some((cols, rows)) {
			return;
		}

		self.document_viewport_cells = Some((cols, rows));
		self.directive = self.runtime.block_on(self.editor.on_event(RuntimeEvent::WindowResized { cols, rows }));
		self.rebuild_snapshot();
	}

	fn forward_document_mouse_event(&mut self, mouse_event: mouse::Event) {
		if let Some(runtime_event) = map_event(Event::Mouse(mouse_event), self.cell_metrics, &mut self.event_state) {
			self.directive = self.runtime.block_on(self.editor.on_event(runtime_event));
			self.rebuild_snapshot();
		}
	}

	fn rect_px(&self, rect: CoreRect) -> (f32, f32, f32, f32) {
		let cw = self.cell_metrics.width_px();
		let ch = self.cell_metrics.height_px();
		(
			f32::from(rect.x) * cw,
			f32::from(rect.y) * ch,
			f32::from(rect.width) * cw,
			f32::from(rect.height) * ch,
		)
	}

	fn separator_theme_colors(&self) -> SeparatorThemeColors {
		let colors = &self.editor.config().theme.colors;
		SeparatorThemeColors {
			base_fg: [map_ui_color_or_default(colors.ui.gutter_fg), map_ui_color_or_default(colors.popup.fg)],
			base_bg: [map_ui_color_or_default(colors.ui.bg), map_ui_color_or_default(colors.popup.bg)],
			hover_fg: map_ui_color_or_default(colors.ui.cursor_fg),
			hover_bg: map_ui_color_or_default(colors.ui.selection_bg),
			drag_fg: map_ui_color_or_default(colors.ui.bg),
			drag_bg: map_ui_color_or_default(colors.ui.fg),
		}
	}

	fn palette_completion_overlay(&self) -> Option<PaletteCompletionOverlay> {
		let target = self.editor.overlay_completion_menu_target()?;
		let rect = target.rect();
		let x_px = f32::from(rect.x) * self.cell_metrics.width_px();
		let y_px = f32::from(rect.y) * self.cell_metrics.height_px();
		let width_px = f32::from(rect.width) * self.cell_metrics.width_px();

		Some(PaletteCompletionOverlay {
			x_px,
			y_px,
			width_px,
			plan: target.plan().clone(),
		})
	}
}

struct SeparatorThemeColors {
	base_fg: [iced::Color; 2],
	base_bg: [iced::Color; 2],
	hover_fg: iced::Color,
	hover_bg: iced::Color,
	drag_fg: iced::Color,
	drag_bg: iced::Color,
}

fn separator_fg_bg(state: &xeno_editor::render_api::SeparatorState, priority: u8, colors: &SeparatorThemeColors) -> (iced::Color, iced::Color) {
	let idx = (priority as usize).min(colors.base_fg.len() - 1);
	let normal_fg = colors.base_fg[idx];
	let normal_bg = colors.base_bg[idx];

	if state.is_dragging() {
		(colors.drag_fg, colors.drag_bg)
	} else if state.is_animating() {
		let t = state.anim_intensity();
		(lerp_color(normal_fg, colors.hover_fg, t), lerp_color(normal_bg, colors.hover_bg, t))
	} else if state.is_hovered() {
		(colors.hover_fg, colors.hover_bg)
	} else {
		(normal_fg, normal_bg)
	}
}

fn lerp_color(a: iced::Color, b: iced::Color, t: f32) -> iced::Color {
	iced::Color {
		r: a.r + (b.r - a.r) * t,
		g: a.g + (b.g - a.g) * t,
		b: a.b + (b.b - a.b) * t,
		a: a.a + (b.a - a.a) * t,
	}
}

fn map_ui_color_or_default(color: xeno_primitives::Color) -> iced::Color {
	use self::render::map_ui_color;
	map_ui_color(color).unwrap_or(iced::Color::BLACK)
}

fn viewport_grid_from_document_size(editor: &Editor, cell_metrics: super::CellMetrics, document_size: Size) -> (u16, u16) {
	let (cols, document_rows) = cell_metrics.to_grid(document_size.width, document_size.height);
	(cols, viewport_rows_for_document_rows(editor, document_rows))
}

fn viewport_rows_for_document_rows(editor: &Editor, document_rows: u16) -> u16 {
	document_rows.saturating_add(editor.statusline_rows())
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
