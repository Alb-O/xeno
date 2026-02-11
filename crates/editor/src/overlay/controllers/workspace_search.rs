//! Workspace search overlay (stub).
//!
//! Disabled pending a new search implementation.

use std::future::Future;
use std::pin::Pin;

use termina::event::KeyEvent;
use xeno_registry::notifications::keys;
use xeno_registry::options::OptionValue;

use crate::buffer::ViewId;
use crate::overlay::{CloseReason, OverlayContext, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy, WindowRole, WindowSpec};
use crate::window::GutterSelector;

pub struct WorkspaceSearchOverlay {
	list_buffer: Option<ViewId>,
}

impl Default for WorkspaceSearchOverlay {
	fn default() -> Self {
		Self::new()
	}
}

impl WorkspaceSearchOverlay {
	pub fn new() -> Self {
		Self { list_buffer: None }
	}

	fn list_buffer_id(&self, session: &OverlaySession) -> Option<ViewId> {
		self.list_buffer.or_else(|| session.buffers.iter().copied().find(|id| *id != session.input))
	}

	fn set_list_content(&self, ctx: &mut dyn OverlayContext, session: &OverlaySession, content: String) {
		let Some(buffer_id) = self.list_buffer_id(session) else {
			return;
		};
		ctx.reset_buffer_content(buffer_id, &content);
	}
}

impl OverlayController for WorkspaceSearchOverlay {
	fn name(&self) -> &'static str {
		"WorkspaceSearch"
	}

	fn ui_spec(&self, _ctx: &dyn OverlayContext) -> OverlayUiSpec {
		let mut buffer_options = std::collections::HashMap::new();
		buffer_options.insert("cursorline".into(), OptionValue::Bool(false));

		OverlayUiSpec {
			title: Some("Workspace Search".into()),
			gutter: GutterSelector::Prompt('/'),
			rect: RectPolicy::TopCenter {
				width_percent: 100,
				max_width: u16::MAX,
				min_width: 1,
				y_frac: (0, 1),
				height: 1,
			},
			style: crate::overlay::docked_prompt_style(),
			windows: vec![WindowSpec {
				role: WindowRole::List,
				rect: RectPolicy::Below(WindowRole::Input, 1, 9),
				style: crate::overlay::docked_prompt_style(),
				buffer_options,
				dismiss_on_blur: false,
				sticky: false,
				gutter: GutterSelector::Hidden,
			}],
		}
	}

	fn on_open(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession) {
		self.list_buffer = session.buffers.iter().copied().find(|id| *id != session.input);
		self.set_list_content(ctx, session, "Workspace search is not available".to_string());
	}

	fn on_input_changed(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _text: &str) {}

	fn on_key(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _key: KeyEvent) -> bool {
		false
	}

	fn on_commit<'a>(&'a mut self, ctx: &'a mut dyn OverlayContext, _session: &'a mut OverlaySession) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		Box::pin(async move {
			ctx.notify(keys::info("Workspace search is not available"));
		})
	}

	fn on_close(&mut self, _ctx: &mut dyn OverlayContext, _session: &mut OverlaySession, _reason: CloseReason) {}
}
