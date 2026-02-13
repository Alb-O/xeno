use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Event, Fill, Font, Subscription, Task, event, keyboard, time, window};
use xeno_editor::Editor;
use xeno_editor::runtime::{CursorStyle, LoopDirective, RuntimeEvent};

use super::{DEFAULT_POLL_INTERVAL, EventBridgeState, Snapshot, StartupOptions, build_snapshot, configure_linux_backend, map_event};

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

		let document = container(scrollable(text(&self.snapshot.body).font(Font::MONOSPACE)).height(Fill))
			.width(Fill)
			.height(Fill);
		let inspector = container(scrollable(text(&self.snapshot.inspector).font(Font::MONOSPACE)).height(Fill))
			.width(320)
			.height(Fill);
		let panes = row![document, inspector].spacing(12).height(Fill);
		let statusline = text(&self.snapshot.statusline).font(Font::MONOSPACE);

		let content = column![header_block, panes, statusline].spacing(8).padding(12).height(Fill);

		container(content).width(Fill).height(Fill).into()
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

pub fn run(startup: StartupOptions) -> iced::Result {
	configure_linux_backend();

	iced::application(move || IcedEditorApp::boot(startup.clone()), IcedEditorApp::update, IcedEditorApp::view)
		.title(IcedEditorApp::title)
		.subscription(IcedEditorApp::subscription)
		.window_size((1200.0, 800.0))
		.run()
}

#[cfg(test)]
mod tests {
	use super::*;

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
}
