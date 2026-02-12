use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use xeno_primitives::{Key, KeyCode};
use xeno_registry::notifications::Notification;

use crate::buffer::{Buffer, ViewId};

pub mod controllers;
pub(crate) mod geom;
pub mod host;
pub mod session;
pub mod spec;

#[cfg(test)]
mod invariants;

pub use host::OverlayHost;
pub use session::*;
pub use spec::*;
use xeno_tui::widgets::BorderType;
use xeno_tui::widgets::block::Padding;

use crate::window::SurfaceStyle;

/// Helper to create a docked, inline prompt style for utility panel overlays.
pub fn docked_prompt_style() -> SurfaceStyle {
	SurfaceStyle {
		border: false,
		border_type: BorderType::Stripe,
		padding: Padding::horizontal(1),
		shadow: false,
		title: None,
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
	pub fn get<T>(&self) -> Option<&T>
	where
		T: Any + Send + Sync,
	{
		self.inner.get(&TypeId::of::<T>())?.downcast_ref()
	}

	/// Returns a mutable reference to a stored value of type `T`.
	pub fn get_mut<T>(&mut self) -> Option<&mut T>
	where
		T: Any + Send + Sync,
	{
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
	pub fn get_or_default<T>(&mut self) -> &mut T
	where
		T: Any + Send + Sync + Default,
	{
		let type_id = TypeId::of::<T>();
		let slot = self.inner.entry(type_id).or_insert_with(|| Box::<T>::default());

		slot.downcast_mut::<T>()
			.expect("OverlayStore invariant violation: TypeId present with non-matching concrete type")
	}

	/// Inserts a value of type `T` into the store.
	pub fn insert<T>(&mut self, val: T)
	where
		T: Any + Send + Sync,
	{
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
	active: Option<ActiveOverlay>,
}

/// Coupling of a modal session's resources and its behavioral controller.
pub struct ActiveOverlay {
	/// Low-level UI resources (buffers, panes) allocated for this interaction.
	pub session: OverlaySession,
	/// High-level logic governing the interaction's behavior.
	pub controller: Box<dyn OverlayController>,
}

/// Unified overlay system managing modal interactions, passive layers, and shared state.
///
/// The `OverlaySystem` orchestrates two primary types of UI overlays:
/// 1. Modal Interactions: Managed by [`OverlayManager`], these are focus-stealing
///    activities like command palette or search prompts that usually involve
///    a dedicated input buffer.
/// 2. Passive Layers: Managed by [`OverlayLayers`], these are non-focusing
///    contextual elements like info tooltips, diagnostics popovers, or LSP
///    completion menus.
///
/// It also provides a type-erased [`OverlayStore`] for sharing passive state
/// between the editor and various layers.
pub struct OverlaySystem {
	/// Manager for focus-stealing modal interaction sessions.
	interaction: OverlayManager,
	/// Stack of passive, contextual UI layers.
	layers: OverlayLayers,
	/// Type-erased storage for shared overlay data.
	store: OverlayStore,
}

impl OverlaySystem {
	/// Creates a new `OverlaySystem` with default layers initialized.
	///
	/// Initial layers include:
	/// - [`controllers::InfoPopupLayer`] for event-driven info popup dismissal.
	pub fn new() -> Self {
		let mut layers = OverlayLayers::default();
		layers.add(Box::new(controllers::InfoPopupLayer));
		Self {
			interaction: OverlayManager::default(),
			layers,
			store: OverlayStore::default(),
		}
	}

	pub fn interaction(&self) -> &OverlayManager {
		&self.interaction
	}

	pub fn interaction_mut(&mut self) -> &mut OverlayManager {
		&mut self.interaction
	}

	pub fn take_interaction(&mut self) -> OverlayManager {
		std::mem::take(&mut self.interaction)
	}

	pub fn restore_interaction(&mut self, interaction: OverlayManager) {
		self.interaction = interaction;
	}

	pub fn layers(&self) -> &OverlayLayers {
		&self.layers
	}

	pub fn layers_mut(&mut self) -> &mut OverlayLayers {
		&mut self.layers
	}

	pub fn store(&self) -> &OverlayStore {
		&self.store
	}

	pub fn store_mut(&mut self) -> &mut OverlayStore {
		&mut self.store
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

/// Capability interface for overlay controllers.
///
/// This intentionally exposes a limited surface area relative to the full
/// editor API, while still allowing overlays to perform their work.
pub trait OverlayContext {
	/// Returns a buffer by view ID.
	fn buffer(&self, id: ViewId) -> Option<&Buffer>;
	/// Returns a mutable buffer by view ID.
	fn buffer_mut(&mut self, id: ViewId) -> Option<&mut Buffer>;
	/// Replaces a buffer's content and resets syntax state.
	fn reset_buffer_content(&mut self, view: ViewId, content: &str);
	/// Emits a user-visible notification.
	fn notify(&mut self, notification: Notification);
	/// Reveals the cursor for a view.
	fn reveal_cursor_in_view(&mut self, view: ViewId);
	/// Requests a redraw for the next frame.
	fn request_redraw(&mut self);
	/// Queues a command by name.
	fn queue_command(&mut self, name: &'static str, args: Vec<String>);
	/// Returns the async message sender for background results.
	fn msg_tx(&self) -> crate::msg::MsgSender;
	/// Finalizes removal for a buffer.
	fn finalize_buffer_removal(&mut self, view: ViewId);
	/// Returns completion state when available.
	fn completion_state(&self) -> Option<&crate::completion::CompletionState>;
	/// Returns mutable completion state, creating one when absent.
	fn completion_state_mut(&mut self) -> &mut crate::completion::CompletionState;
	/// Clears completion state to inactive defaults.
	fn clear_completion_state(&mut self);
	/// Records command usage for palette ranking.
	fn record_command_usage(&mut self, canonical: &str);
	/// Returns a snapshot of command usage state.
	fn command_usage_snapshot(&self) -> crate::completion::CommandUsageSnapshot;
	/// Returns filesystem indexing/search service state.
	fn filesystem(&self) -> &crate::filesystem::FsService;
	/// Returns mutable filesystem indexing/search service state.
	fn filesystem_mut(&mut self) -> &mut crate::filesystem::FsService;

	#[cfg(feature = "lsp")]
	fn lsp_prepare_position_request(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<(xeno_lsp::ClientHandle, xeno_lsp::lsp_types::Uri, xeno_lsp::lsp_types::Position)>>;

	#[cfg(feature = "lsp")]
	fn apply_workspace_edit<'a>(
		&'a mut self,
		edit: xeno_lsp::lsp_types::WorkspaceEdit,
	) -> Pin<Box<dyn Future<Output = Result<(), crate::lsp::workspace_edit::ApplyError>> + 'a>>;
}

/// Behavioral logic for a modal interaction session.
pub trait OverlayController: Send + Sync {
	/// Stable identifier for the controller kind.
	fn name(&self) -> &'static str;

	/// Defines the initial UI configuration for the session.
	fn ui_spec(&self, ctx: &dyn OverlayContext) -> OverlayUiSpec;

	/// Called immediately after the session resources are allocated.
	fn on_open(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession);

	/// Called when the primary input buffer content changes.
	fn on_input_changed(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, text: &str);

	/// Processes raw key events. Returns `true` if the event was handled.
	fn on_key(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, key: Key) -> bool {
		let _ = (ctx, session, key);
		false
	}

	/// Performs the interaction's final action. Called when the session is committed.
	fn on_commit<'a>(&'a mut self, ctx: &'a mut dyn OverlayContext, session: &'a mut OverlaySession) -> Pin<Box<dyn Future<Output = ()> + 'a>>;

	/// Final cleanup hook. Called when the session is closed for any reason.
	fn on_close(&mut self, ctx: &mut dyn OverlayContext, session: &mut OverlaySession, reason: CloseReason);
}

impl OverlayContext for crate::impls::Editor {
	fn buffer(&self, id: ViewId) -> Option<&Buffer> {
		self.state.core.buffers.get_buffer(id)
	}

	fn buffer_mut(&mut self, id: ViewId) -> Option<&mut Buffer> {
		self.state.core.buffers.get_buffer_mut(id)
	}

	fn reset_buffer_content(&mut self, view: ViewId, content: &str) {
		let Some(buffer) = self.state.core.buffers.get_buffer_mut(view) else {
			return;
		};
		buffer.reset_content(content);
		self.state.syntax_manager.reset_syntax(buffer.document_id());
	}

	fn notify(&mut self, notification: Notification) {
		self.notify(notification);
	}

	fn reveal_cursor_in_view(&mut self, view: ViewId) {
		self.reveal_cursor_in_view(view);
	}

	fn request_redraw(&mut self) {
		self.state.frame.needs_redraw = true;
	}

	fn queue_command(&mut self, name: &'static str, args: Vec<String>) {
		self.state.core.workspace.command_queue.push(name, args);
	}

	fn msg_tx(&self) -> crate::msg::MsgSender {
		self.msg_tx()
	}

	fn finalize_buffer_removal(&mut self, view: ViewId) {
		self.finalize_buffer_removal(view);
	}

	fn completion_state(&self) -> Option<&crate::completion::CompletionState> {
		self.overlays().get::<crate::completion::CompletionState>()
	}

	fn completion_state_mut(&mut self) -> &mut crate::completion::CompletionState {
		self.overlays_mut().get_or_default::<crate::completion::CompletionState>()
	}

	fn clear_completion_state(&mut self) {
		let state = self.overlays_mut().get_or_default::<crate::completion::CompletionState>();
		*state = crate::completion::CompletionState::default();
	}

	fn record_command_usage(&mut self, canonical: &str) {
		self.state.command_usage.record(canonical);
	}

	fn command_usage_snapshot(&self) -> crate::completion::CommandUsageSnapshot {
		self.state.command_usage.snapshot()
	}

	fn filesystem(&self) -> &crate::filesystem::FsService {
		&self.state.filesystem
	}

	fn filesystem_mut(&mut self) -> &mut crate::filesystem::FsService {
		&mut self.state.filesystem
	}

	#[cfg(feature = "lsp")]
	fn lsp_prepare_position_request(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<(xeno_lsp::ClientHandle, xeno_lsp::lsp_types::Uri, xeno_lsp::lsp_types::Position)>> {
		self.state.lsp.prepare_position_request(buffer)
	}

	#[cfg(feature = "lsp")]
	fn apply_workspace_edit<'a>(
		&'a mut self,
		edit: xeno_lsp::lsp_types::WorkspaceEdit,
	) -> Pin<Box<dyn Future<Output = Result<(), crate::lsp::workspace_edit::ApplyError>> + 'a>> {
		Box::pin(self.apply_workspace_edit(edit))
	}
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
	fn layout(&self, ed: &crate::impls::Editor, screen: crate::geometry::Rect) -> Option<crate::geometry::Rect>;

	/// Renders the layer content into the terminal frame.
	fn render(&self, ed: &crate::impls::Editor, frame: &mut xeno_tui::Frame, area: crate::geometry::Rect);

	/// Optional key interception for visible layers (e.g. Tab/Enter in completion menus).
	fn on_key(&mut self, _ed: &mut crate::impls::Editor, _key: Key) -> bool {
		false
	}

	/// Notifies the layer about editor state changes.
	fn on_event(&mut self, _ed: &mut crate::impls::Editor, _event: &LayerEvent) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerEvent {
	/// Primary cursor moved in the focused buffer.
	CursorMoved { view: ViewId },
	/// Global editor mode changed (e.g. Insert -> Normal).
	ModeChanged { view: ViewId, mode: xeno_primitives::Mode },
	/// Content of a buffer was modified.
	BufferEdited(ViewId),
	/// Focus shifted between windows or panels.
	FocusChanged {
		from: crate::impls::FocusTarget,
		to: crate::impls::FocusTarget,
	},
	/// The editor layout (splits, window sizes) has changed.
	LayoutChanged,
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
	pub fn handle_key(&mut self, ed: &mut crate::impls::Editor, key: Key) -> bool {
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
			layer.on_event(ed, &event);
		}
	}

	/// Renders all visible layers in stack order.
	pub fn render(&self, ed: &crate::impls::Editor, frame: &mut xeno_tui::Frame) {
		let screen = match (ed.state.viewport.width, ed.state.viewport.height) {
			(Some(w), Some(h)) => crate::geometry::Rect::new(0, 0, w, h),
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
	pub fn active(&self) -> Option<&ActiveOverlay> {
		self.active.as_ref()
	}

	pub fn active_mut(&mut self) -> Option<&mut ActiveOverlay> {
		self.active.as_mut()
	}

	/// Returns `true` if a modal interaction is currently active.
	pub fn is_open(&self) -> bool {
		self.active.is_some()
	}

	/// Starts a new modal interaction session.
	///
	/// Fails and returns `false` if an interaction is already active.
	pub fn open(&mut self, ed: &mut crate::impls::Editor, mut controller: Box<dyn OverlayController>) -> bool {
		if self.is_open() {
			return false;
		}

		let spec = controller.ui_spec(ed);
		let desired_height = if controller.name() == "CommandPalette" || spec.windows.is_empty() {
			1
		} else {
			10
		};
		ed.state.ui.sync_utility_for_modal_overlay(Some(desired_height));

		if let Some(mut session) = OverlayHost::setup_session(ed, &*controller) {
			#[cfg(feature = "lsp")]
			ed.clear_lsp_menu();

			controller.on_open(ed, &mut session);
			self.active = Some(ActiveOverlay { session, controller });
			true
		} else {
			ed.state.ui.sync_utility_for_modal_overlay(None);
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
			OverlayHost::cleanup_session(ed, &mut *active.controller, active.session, CloseReason::Commit);
		}
	}

	/// Routes key events to the active interaction.
	///
	/// Falls back to default host dismissal (Esc -> Cancel) if the controller
	/// does not handle the key.
	pub fn handle_key(&mut self, ed: &mut crate::impls::Editor, key: Key) -> bool {
		let Some(active) = self.active.as_mut() else {
			return false;
		};

		if active.controller.on_key(ed, &mut active.session, key) {
			return true;
		}

		match key.code {
			KeyCode::Esc => {
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
		active.controller.on_input_changed(ed, &mut active.session, &text);
	}

	/// Triggers a controller refresh by replaying current input text.
	pub fn refresh_if(&mut self, ed: &mut crate::impls::Editor, name: &'static str) {
		let Some(active) = self.active.as_mut() else {
			return;
		};
		if active.controller.name() != name {
			return;
		}

		let text = active.session.input_text(ed);
		active.controller.on_input_changed(ed, &mut active.session, &text);
	}

	/// Called when terminal viewport dimensions change.
	pub fn on_viewport_changed(&mut self, ed: &mut crate::impls::Editor) {
		let Some(mut active) = self.active.take() else {
			return;
		};

		if OverlayHost::reflow_session(ed, &*active.controller, &mut active.session) {
			ed.state.frame.needs_redraw = true;
			self.active = Some(active);
			return;
		}

		OverlayHost::cleanup_session(ed, &mut *active.controller, active.session, CloseReason::Forced);
	}
}

#[cfg(test)]
mod tests;
