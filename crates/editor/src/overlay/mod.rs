use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use termina::event::{KeyCode, KeyEvent};

use crate::buffer::ViewId;

pub mod controllers;
pub mod host;
pub mod session;
pub mod spec;

pub use host::OverlayHost;
pub use session::*;
pub use spec::*;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::window::FloatingStyle;

pub fn prompt_style(title: &str) -> FloatingStyle {
	FloatingStyle {
		border: true,
		border_type: BorderType::Stripe,
		padding: Padding::horizontal(1),
		shadow: false,
		title: Some(title.to_string()),
	}
}

/// Passive type-erased storage for non-interactive overlays (completions, popups).
#[derive(Default)]
pub struct OverlayStore {
	inner: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl OverlayStore {
	pub fn new() -> Self {
		Self::default()
	}
	pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
		self.inner.get(&TypeId::of::<T>())?.downcast_ref()
	}
	pub fn get_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
		self.inner.get_mut(&TypeId::of::<T>())?.downcast_mut()
	}
	pub fn get_or_default<T: Any + Send + Sync + Default>(&mut self) -> &mut T {
		let type_id = TypeId::of::<T>();
		self.inner
			.entry(type_id)
			.or_insert_with(|| Box::<T>::default());
		self.inner
			.get_mut(&type_id)
			.unwrap()
			.downcast_mut()
			.unwrap()
	}
	pub fn insert<T: Any + Send + Sync>(&mut self, val: T) {
		self.inner.insert(TypeId::of::<T>(), Box::new(val));
	}
}

/// Active interaction manager for search, palette, etc.
#[derive(Default)]
pub struct OverlayManager {
	pub session: Option<OverlaySession>,
	pub controller: Option<Box<dyn OverlayController>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
	Cancel,
	Commit,
	Blur,
	Forced,
}

pub trait OverlayController: Send + Sync {
	fn name(&self) -> &'static str;
	fn ui_spec(&self, ed: &crate::impls::Editor) -> OverlayUiSpec;

	fn on_open(&mut self, ed: &mut crate::impls::Editor, session: &mut OverlaySession);
	fn on_input_changed(
		&mut self,
		ed: &mut crate::impls::Editor,
		session: &mut OverlaySession,
		text: &str,
	);

	fn on_key(
		&mut self,
		ed: &mut crate::impls::Editor,
		session: &mut OverlaySession,
		key: KeyEvent,
	) -> bool {
		let _ = (ed, session, key);
		false
	}

	fn on_commit<'a>(
		&'a mut self,
		ed: &'a mut crate::impls::Editor,
		session: &'a mut OverlaySession,
	) -> Pin<Box<dyn Future<Output = ()> + 'a>>;

	fn on_close(
		&mut self,
		ed: &mut crate::impls::Editor,
		session: &mut OverlaySession,
		reason: CloseReason,
	);
}

impl OverlayManager {
	pub fn is_open(&self) -> bool {
		self.session.is_some()
	}

	pub fn open(
		&mut self,
		ed: &mut crate::impls::Editor,
		mut controller: Box<dyn OverlayController>,
	) -> bool {
		if self.is_open() {
			return false;
		}

		if let Some(mut session) = OverlayHost::setup_session(ed, &*controller) {
			controller.on_open(ed, &mut session);
			self.session = Some(session);
			self.controller = Some(controller);
			true
		} else {
			false
		}
	}

	pub fn close(&mut self, ed: &mut crate::impls::Editor, reason: CloseReason) {
		let session = self.session.take();
		let controller = self.controller.take();
		if let (Some(session), Some(mut controller)) = (session, controller) {
			OverlayHost::cleanup_session(ed, &mut *controller, session, reason);
		}
	}

	pub async fn commit(&mut self, ed: &mut crate::impls::Editor) {
		let session = self.session.take();
		let controller = self.controller.take();
		if let (Some(mut session), Some(mut controller)) = (session, controller) {
			controller.on_commit(ed, &mut session).await;
			OverlayHost::cleanup_session(ed, &mut *controller, session, CloseReason::Commit);
		}
	}

	pub fn handle_key(&mut self, ed: &mut crate::impls::Editor, key: KeyEvent) -> bool {
		let Some(session) = self.session.as_mut() else {
			return false;
		};
		let Some(controller) = self.controller.as_mut() else {
			return false;
		};

		if controller.on_key(ed, session, key) {
			return true;
		}

		// Default host behavior
		match key.code {
			KeyCode::Escape => {
				self.close(ed, CloseReason::Cancel);
				true
			}
			_ => false,
		}
	}

	pub fn on_buffer_edited(&mut self, ed: &mut crate::impls::Editor, view_id: ViewId) {
		let Some(session) = self.session.as_mut() else {
			return;
		};
		let Some(controller) = self.controller.as_mut() else {
			return;
		};
		if session.input != view_id {
			return;
		}

		let text = session.input_text(ed);
		controller.on_input_changed(ed, session, &text);
	}

	pub fn on_viewport_changed(&mut self, _ed: &mut crate::impls::Editor) {
		// TODO: implement reflow logic in host
	}
}
