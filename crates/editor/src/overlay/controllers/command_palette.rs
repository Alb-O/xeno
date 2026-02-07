use std::future::Future;
use std::pin::Pin;

use xeno_registry::notifications::keys;
use xeno_registry::options::{OptionValue, keys as opt_keys};

use crate::overlay::{
	CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy,
};
use crate::window::GutterSelector;

pub struct CommandPaletteOverlay;

impl Default for CommandPaletteOverlay {
	fn default() -> Self {
		Self::new()
	}
}

impl CommandPaletteOverlay {
	pub fn new() -> Self {
		Self
	}
}

impl OverlayController for CommandPaletteOverlay {
	fn name(&self) -> &'static str {
		"CommandPalette"
	}

	fn ui_spec(&self, _ctx: &dyn OverlayContext) -> OverlayUiSpec {
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

	fn on_open(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) {
		if let Some(buffer) = ctx.buffer_mut(session.input) {
			buffer
				.local_options
				.set(opt_keys::CURSORLINE.untyped(), OptionValue::Bool(false));
		}
	}

	fn on_input_changed(
		&mut self,
		_ctx: &mut dyn OverlayContext,
		_session: &mut OverlaySession,
		_text: &str,
	) {
	}

	fn on_commit<'a>(
		&'a mut self,
		ctx: &'a mut dyn OverlayContext,
		session: &'a mut OverlaySession,
	) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		let input = session.input_text(ctx).trim().to_string();

		if !input.is_empty() {
			let mut parts = input.split_whitespace();
			if let Some(name) = parts.next() {
				let args: Vec<String> = parts.map(String::from).collect();

				if let Some(cmd) = crate::commands::find_editor_command(name) {
					ctx.queue_command(cmd.name, args);
				} else if let Some(cmd) = xeno_registry::commands::find_command(name) {
					let name: &'static str = Box::leak(cmd.name_str().to_string().into_boxed_str());
					ctx.queue_command(name, args);
				} else {
					ctx.notify(keys::unknown_command(name));
				}
			}
		}

		Box::pin(async {})
	}

	fn on_close(
		&mut self,
		_ctx: &mut dyn OverlayContext,
		_session: &mut OverlaySession,
		_reason: CloseReason,
	) {
	}
}
