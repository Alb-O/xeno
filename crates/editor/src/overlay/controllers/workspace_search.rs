//! Workspace search overlay (stub).
//!
//! Disabled pending a new search implementation.

use std::future::Future;
use std::pin::Pin;

use termina::event::KeyEvent;
use xeno_registry::notifications::keys;
use xeno_registry::options::OptionValue;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::buffer::ViewId;
use crate::impls::Editor;
use crate::overlay::{
	CloseReason, OverlayController, OverlaySession, OverlayUiSpec, RectPolicy, WindowRole,
	WindowSpec,
};
use crate::window::{FloatingStyle, GutterSelector};

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
		self.list_buffer.or_else(|| {
			session
				.buffers
				.iter()
				.copied()
				.find(|id| *id != session.input)
		})
	}

	fn set_list_content(&self, ed: &mut Editor, session: &OverlaySession, content: String) {
		let Some(buffer_id) = self.list_buffer_id(session) else {
			return;
		};
		if let Some(buffer) = ed.state.core.buffers.get_buffer_mut(buffer_id) {
			buffer.reset_content(content);
		}
	}
}

impl OverlayController for WorkspaceSearchOverlay {
	fn name(&self) -> &'static str {
		"WorkspaceSearch"
	}

	fn ui_spec(&self, _ed: &Editor) -> OverlayUiSpec {
		let mut buffer_options = std::collections::HashMap::new();
		buffer_options.insert("cursorline".into(), OptionValue::Bool(false));

		OverlayUiSpec {
			title: Some("Workspace Search".into()),
			gutter: GutterSelector::Prompt('/'),
			rect: RectPolicy::TopCenter {
				width_percent: 70,
				max_width: 100,
				min_width: 50,
				y_frac: (1, 6),
				height: 3,
			},
			style: crate::overlay::prompt_style("Workspace Search"),
			windows: vec![WindowSpec {
				role: WindowRole::List,
				rect: RectPolicy::Below(WindowRole::Input, 0, 15),
				style: FloatingStyle {
					border: true,
					border_type: BorderType::Rounded,
					padding: Padding::ZERO,
					shadow: false,
					title: None,
				},
				buffer_options,
				dismiss_on_blur: false,
				sticky: false,
				gutter: GutterSelector::Hidden,
			}],
		}
	}

	fn on_open(&mut self, ed: &mut Editor, session: &mut OverlaySession) {
		self.list_buffer = session
			.buffers
			.iter()
			.copied()
			.find(|id| *id != session.input);
		self.set_list_content(ed, session, "Workspace search is not available".to_string());
	}

	fn on_input_changed(&mut self, _ed: &mut Editor, _session: &mut OverlaySession, _text: &str) {}

	fn on_key(&mut self, _ed: &mut Editor, _session: &mut OverlaySession, _key: KeyEvent) -> bool {
		false
	}

	fn on_commit<'a>(
		&'a mut self,
		ed: &'a mut Editor,
		_session: &'a mut OverlaySession,
	) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
		Box::pin(async move {
			ed.notify(keys::info("Workspace search is not available"));
		})
	}

	fn on_close(&mut self, _ed: &mut Editor, _session: &mut OverlaySession, _reason: CloseReason) {}
}
