use std::future::Future;
use std::pin::Pin;

use xeno_registry::notifications::keys;
use xeno_registry::options::{OptionValue, keys as opt_keys};

use crate::impls::Editor;
use crate::overlay::{CloseReason, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy};
use crate::window::GutterSelector;

pub struct CommandPaletteOverlay;

impl CommandPaletteOverlay {
	pub fn new() -> Self {
		Self
	}
}

impl OverlayController for CommandPaletteOverlay {
	fn name(&self) -> &'static str {
		"CommandPalette"
	}

	fn ui_spec(&self, _ed: &Editor) -> OverlayUiSpec {
		OverlayUiSpec {
			title: None,
			gutter: GutterSelector::Prompt('>'),
			rect: RectPolicy::TopCenter {
				width_percent: 60,
				max_width: 80,
				min_width: 40,
				y_frac: (1, 5),
				height: 3,
			},
			style: crate::overlay::prompt_style("Command Palette"),
			windows: vec![],
		}
	}

	fn on_open(&mut self, ed: &mut Editor, session: &mut OverlaySession) {
		if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(session.input) {
			buffer
				.local_options
				.set(opt_keys::CURSORLINE.untyped(), OptionValue::Bool(false));
		}
	}

	fn on_input_changed(&mut self, _ed: &mut Editor, _session: &mut OverlaySession, _text: &str) {}

	fn on_commit<'a>(
		&'a mut self,
		ed: &'a mut Editor,
		session: &'a mut OverlaySession,
	) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let input = session.input_text(ed).trim().to_string();

		if !input.is_empty() {
			let mut parts = input.split_whitespace();
			if let Some(name) = parts.next() {
				let args: Vec<String> = parts.map(String::from).collect();

				if let Some(cmd) = crate::commands::find_editor_command(name) {
					ed.state.core.workspace.command_queue.push(cmd.name, args);
				} else if let Some(cmd) = xeno_registry::commands::find_command(name) {
					ed.state.core.workspace.command_queue.push(cmd.name(), args);
				} else {
					ed.notify(keys::unknown_command(name));
				}
			}
		}

		Box::pin(async {})
	}

	fn on_close(&mut self, _ed: &mut Editor, _session: &mut OverlaySession, _reason: CloseReason) {}
}
