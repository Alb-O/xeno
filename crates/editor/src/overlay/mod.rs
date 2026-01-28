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

/// Helper to create a consistent floating style for prompt windows.
pub fn prompt_style(title: &str) -> FloatingStyle {
	FloatingStyle {
		border: true,
		border_type: BorderType::Stripe,
		padding: Padding::horizontal(1),
		shadow: false,
		title: Some(title.to_string()),
	}
}

/// Passive type-erased storage for non-interactive overlays.
///
/// Used to store state for completions, signature help, or ad-hoc popups
/// without adding dedicated fields to the core editor state.
#[derive(Default)]
pub struct OverlayStore {
	inner: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl OverlayStore {
	/// Creates an empty store.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns a reference to a stored value of type `T`.
	pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
		self.inner.get(&TypeId::of::<T>())?.downcast_ref()
	}

	/// Returns a mutable reference to a stored value of type `T`.
	pub fn get_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
		self.inner.get_mut(&TypeId::of::<T>())?.downcast_mut()
	}

	/// Returns a mutable reference to a stored value of type `T`,
	/// inserting a default value if it doesn't exist.
	///
	/// # Panics
	///
	/// Panics if a value of a different type is already stored for `TypeId::of::<T>()`.
	/// This is an invariant violation indicating that multiple types are attempting
	/// to use the same `TypeId` slot, which should be impossible in safe Rust.
	pub fn get_or_default<T: Any + Send + Sync + Default>(&mut self) -> &mut T {
		let type_id = TypeId::of::<T>();
		let slot = self
			.inner
			.entry(type_id)
			.or_insert_with(|| Box::<T>::default());

		slot.downcast_mut::<T>().expect(
			"OverlayStore invariant violation: TypeId present with non-matching concrete type",
		)
	}

	/// Inserts a value of type `T` into the store.
	pub fn insert<T: Any + Send + Sync>(&mut self, val: T) {
		self.inner.insert(TypeId::of::<T>(), Box::new(val));
	}
}

/// Active interaction manager for focus-stealing modal overlays.
///
/// Ensures only one modal interaction is active at a time and handles
/// the coupling between the [`OverlaySession`] resources and the
/// [`OverlayController`] logic.
#[derive(Default)]
pub struct OverlayManager {
	/// The currently active modal interaction, if any.
	pub active: Option<ActiveOverlay>,
}

/// Coupling of a modal session's resources and its behavioral controller.
pub struct ActiveOverlay {
	/// Low-level UI resources (buffers, windows) allocated for this interaction.
	pub session: OverlaySession,
	/// High-level logic governing the interaction's behavior.
	pub controller: Box<dyn OverlayController>,
}

/// Unified overlay system managing modal interactions, passive layers, and shared state.
///
/// The `OverlaySystem` orchestrates two primary types of UI overlays:
/// 1. **Modal Interactions**: Managed by [`OverlayManager`], these are focus-stealing
///    activities like command palette or search prompts that usually involve
///    a dedicated input buffer.
/// 2. **Passive Layers**: Managed by [`OverlayLayers`], these are non-focusing
///    contextual elements like info tooltips, diagnostics popovers, or LSP
///    completion menus.
///
/// It also provides a type-erased [`OverlayStore`] for sharing passive state
/// between the editor and various layers.
pub struct OverlaySystem {
	/// Manager for focus-stealing modal interaction sessions.
	pub interaction: OverlayManager,
	/// Stack of passive, contextual UI layers.
	pub layers: OverlayLayers,
	/// Type-erased storage for shared overlay data.
	pub store: OverlayStore,
}

impl OverlaySystem {
	/// Creates a new `OverlaySystem` with default layers initialized.
	///
	/// Initial layers include:
	/// - [`controllers::InfoPopupLayer`] for displaying documentation popups.
	pub fn new() -> Self {
		let mut layers = OverlayLayers::default();
		layers.add(Box::new(controllers::InfoPopupLayer));
		Self {
			interaction: OverlayManager::default(),
			layers,
			store: OverlayStore::default(),
		}
	}
}

impl Default for OverlaySystem {
	fn default() -> Self {
		Self::new()
	}
}

/// Reason for terminating an overlay session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
	/// User canceled explicitly (e.g. Esc).
	Cancel,
	/// User accepted/committed the interaction (e.g. Enter).
	Commit,
	/// Interaction lost focus and was automatically dismissed.
	Blur,
	/// Interaction was closed programmatically (e.g. forced cleanup).
	Forced,
}

/// Behavioral logic for a modal interaction session.
pub trait OverlayController: Send + Sync {
	/// Stable identifier for the controller kind.
	fn name(&self) -> &'static str;

	/// Defines the initial UI configuration for the session.
	fn ui_spec(&self, ed: &crate::impls::Editor) -> OverlayUiSpec;

	/// Called immediately after the session resources are allocated.
	fn on_open(&mut self, ed: &mut crate::impls::Editor, session: &mut OverlaySession);

	/// Called when the primary input buffer content changes.
	fn on_input_changed(
		&mut self,
		ed: &mut crate::impls::Editor,
		session: &mut OverlaySession,
		text: &str,
	);

	/// Processes raw key events. Returns `true` if the event was handled.
	fn on_key(
		&mut self,
		ed: &mut crate::impls::Editor,
		session: &mut OverlaySession,
		key: KeyEvent,
	) -> bool {
		let _ = (ed, session, key);
		false
	}

	/// Performs the interaction's final action. Called when the session is committed.
	fn on_commit<'a>(
		&'a mut self,
		ed: &'a mut crate::impls::Editor,
		session: &'a mut OverlaySession,
	) -> Pin<Box<dyn Future<Output = ()> + 'a>>;

	/// Final cleanup hook. Called when the session is closed for any reason.
	fn on_close(
		&mut self,
		ed: &mut crate::impls::Editor,
		session: &mut OverlaySession,
		reason: CloseReason,
	);
}

/// Trait for passive, contextual UI elements.
///
/// Unlike controllers, layers do not steal focus or own dedicated input buffers.
/// They are used for tooltips, diagnostics, and inline completion menus.
pub trait OverlayLayer: Send + Sync {
	fn name(&self) -> &'static str;

	/// Determines if the layer should be rendered in the current editor state.
	fn is_visible(&self, ed: &crate::impls::Editor) -> bool;

	/// Computes the screen area for the layer based on the current viewport.
	fn layout(
		&self,
		ed: &crate::impls::Editor,
		screen: xeno_tui::layout::Rect,
	) -> Option<xeno_tui::layout::Rect>;

	/// Renders the layer content into the terminal frame.
	fn render(
		&self,
		ed: &crate::impls::Editor,
		frame: &mut xeno_tui::Frame,
		area: xeno_tui::layout::Rect,
	);

	/// Optional key interception for visible layers (e.g. Tab/Enter in completion menus).
	fn on_key(&mut self, _ed: &mut crate::impls::Editor, _key: KeyEvent) -> bool {
		false
	}

	/// Notifies the layer about editor state changes.
	fn on_event(&mut self, _ed: &mut crate::impls::Editor, _event: LayerEvent) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerEvent {
	/// Primary cursor moved in the focused buffer.
	CursorMoved,
	/// Global editor mode changed (e.g. Insert -> Normal).
	ModeChanged,
	/// Content of a buffer was modified.
	BufferEdited(ViewId),
}

impl LayerEvent {
	/// Returns true if the event should be propagated to all layers.
	pub fn is_broadcast(&self) -> bool {
		true
	}
}

/// Collection of passive UI layers with ordered rendering and event propagation.
#[derive(Default)]
pub struct OverlayLayers {
	layers: Vec<Box<dyn OverlayLayer>>,
}

impl OverlayLayers {
	/// Adds a new layer to the top of the stack.
	pub fn add(&mut self, layer: Box<dyn OverlayLayer>) {
		self.layers.push(layer);
	}

	/// Routes key events to visible layers in reverse Z-order.
	pub fn handle_key(&mut self, ed: &mut crate::impls::Editor, key: KeyEvent) -> bool {
		for layer in self.layers.iter_mut().rev() {
			if layer.is_visible(ed) && layer.on_key(ed, key) {
				return true;
			}
		}
		false
	}

	/// Propagates events to all layers.
	pub fn notify_event(&mut self, ed: &mut crate::impls::Editor, event: LayerEvent) {
		for layer in &mut self.layers {
			layer.on_event(ed, event);
		}
	}

	/// Renders all visible layers in stack order.
	pub fn render(&self, ed: &crate::impls::Editor, frame: &mut xeno_tui::Frame) {
		let screen = match (ed.state.viewport.width, ed.state.viewport.height) {
			(Some(w), Some(h)) => xeno_tui::layout::Rect::new(0, 0, w, h),
			_ => return,
		};

		for layer in &self.layers {
			if layer.is_visible(ed)
				&& let Some(area) = layer.layout(ed, screen)
			{
				layer.render(ed, frame, area);
			}
		}
	}
}

impl OverlayManager {
	/// Returns `true` if a modal interaction is currently active.
	pub fn is_open(&self) -> bool {
		self.active.is_some()
	}

	/// Starts a new modal interaction session.
	///
	/// Fails and returns `false` if an interaction is already active.
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
			self.active = Some(ActiveOverlay {
				session,
				controller,
			});
			true
		} else {
			false
		}
	}

	/// Closes the active interaction session with the specified reason.
	pub fn close(&mut self, ed: &mut crate::impls::Editor, reason: CloseReason) {
		if let Some(mut active) = self.active.take() {
			OverlayHost::cleanup_session(ed, &mut *active.controller, active.session, reason);
		}
	}

	/// Commits and terminates the active interaction session.
	pub async fn commit(&mut self, ed: &mut crate::impls::Editor) {
		if let Some(mut active) = self.active.take() {
			active.controller.on_commit(ed, &mut active.session).await;
			OverlayHost::cleanup_session(
				ed,
				&mut *active.controller,
				active.session,
				CloseReason::Commit,
			);
		}
	}

	/// Routes key events to the active interaction.
	///
	/// Falls back to default host dismissal (Esc -> Cancel) if the controller
	/// does not handle the key.
	pub fn handle_key(&mut self, ed: &mut crate::impls::Editor, key: KeyEvent) -> bool {
		let Some(active) = self.active.as_mut() else {
			return false;
		};

		if active.controller.on_key(ed, &mut active.session, key) {
			return true;
		}

		match key.code {
			KeyCode::Escape => {
				self.close(ed, CloseReason::Cancel);
				true
			}
			_ => false,
		}
	}

	/// Routes buffer change notifications to the active interaction.
	pub fn on_buffer_edited(&mut self, ed: &mut crate::impls::Editor, view_id: ViewId) {
		let Some(active) = self.active.as_mut() else {
			return;
		};
		if active.session.input != view_id {
			return;
		}

		let text = active.session.input_text(ed);
		active
			.controller
			.on_input_changed(ed, &mut active.session, &text);
	}

	/// Called when terminal viewport dimensions change.
	pub fn on_viewport_changed(&mut self, _ed: &mut crate::impls::Editor) {
		// TODO: implement reflow logic in host
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn overlay_store_get_or_default_is_stable_and_mutable() {
		#[derive(Default, Debug, PartialEq)]
		struct Foo {
			n: i32,
		}

		let mut s = OverlayStore {
			inner: Default::default(),
		};

		let p1 = {
			let r = s.get_or_default::<Foo>();
			r.n = 7;
			r as *mut Foo
		};

		let p2 = {
			let r = s.get_or_default::<Foo>();
			assert_eq!(r.n, 7);
			r.n = 9;
			r as *mut Foo
		};

		assert_eq!(p1, p2);

		let r = s.get_or_default::<Foo>();
		assert_eq!(r.n, 9);
	}
}
