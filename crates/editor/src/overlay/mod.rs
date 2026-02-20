//! Unified overlay subsystem.
//!
//! Coordinates modal overlay sessions, passive overlay layers, and shared
//! type-erased overlay state used by completion, info popups, and other
//! contextual UI features.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use xeno_primitives::{Key, KeyCode};
use xeno_registry::notifications::Notification;

use crate::buffer::{Buffer, ViewId};
use crate::window::{SurfaceBorder, SurfacePadding, SurfaceStyle};

pub mod controllers;
pub(crate) mod geom;
pub mod host;
pub(crate) mod picker_engine;
pub mod session;
pub mod spec;

pub use host::OverlayHost;
pub use session::*;
pub use spec::*;

/// Helper to create a docked, inline prompt style for utility panel overlays.
pub fn docked_prompt_style() -> SurfaceStyle {
	SurfaceStyle {
		border: false,
		border_type: SurfaceBorder::Stripe,
		padding: SurfacePadding::horizontal(1),
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
	/// Returns a reference to a stored value of type `T`.
	pub fn get<T>(&self) -> Option<&T>
	where
		T: Any + Send + Sync,
	{
		self.inner.get(&TypeId::of::<T>())?.downcast_ref()
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
	/// * [`controllers::InfoPopupLayer`] for event-driven info popup dismissal.
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

	#[cfg(test)]
	pub fn interaction_mut(&mut self) -> &mut OverlayManager {
		&mut self.interaction
	}

	pub fn take_interaction(&mut self) -> OverlayManager {
		std::mem::take(&mut self.interaction)
	}

	pub fn restore_interaction(&mut self, interaction: OverlayManager) {
		self.interaction = interaction;
	}

	#[cfg(test)]
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

/// Stable frontend-facing categorization for active modal controllers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayControllerKind {
	CommandPalette,
	FilePicker,
	Search,
	Rename,
	WorkspaceSearch,
	Other(&'static str),
}

impl OverlayControllerKind {
	pub fn from_name(name: &'static str) -> Self {
		match name {
			"CommandPalette" => Self::CommandPalette,
			"FilePicker" => Self::FilePicker,
			"Search" => Self::Search,
			"Rename" => Self::Rename,
			"WorkspaceSearch" => Self::WorkspaceSearch,
			other => Self::Other(other),
		}
	}

	pub fn default_title(self) -> &'static str {
		match self {
			Self::CommandPalette => "Command Palette",
			Self::FilePicker => "File Picker",
			Self::Search => "Search",
			Self::Rename => "Rename",
			Self::WorkspaceSearch => "Workspace Search",
			Self::Other(other) => other,
		}
	}

	pub fn virtual_buffer_kind(self) -> xeno_buffer_display::VirtualBufferKind {
		match self {
			Self::CommandPalette => xeno_buffer_display::VirtualBufferKind::CommandPalette,
			Self::FilePicker => xeno_buffer_display::VirtualBufferKind::FilePicker,
			Self::Search => xeno_buffer_display::VirtualBufferKind::Search,
			Self::Rename => xeno_buffer_display::VirtualBufferKind::Rename,
			Self::WorkspaceSearch => xeno_buffer_display::VirtualBufferKind::WorkspaceSearch,
			Self::Other(other) => xeno_buffer_display::VirtualBufferKind::OverlayCustom(other.to_string()),
		}
	}
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
	/// Queues a deferred invocation request.
	fn queue_invocation(&mut self, request: xeno_registry::actions::DeferredInvocationRequest);
	/// Returns the shared worker runtime.
	#[cfg(feature = "lsp")]
	fn worker_runtime(&self) -> xeno_worker::WorkerRuntime;
	/// Returns the async message sender for background results.
	#[cfg(feature = "lsp")]
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
	#[allow(dead_code, reason = "overlay controllers do not all consume LSP workspace edits yet")]
	fn apply_workspace_edit<'a>(
		&'a mut self,
		edit: xeno_lsp::lsp_types::WorkspaceEdit,
	) -> Pin<Box<dyn Future<Output = Result<(), crate::lsp::workspace_edit::ApplyError>> + 'a>>;

	/// Generates a monotonic token for rename requests and records it as pending.
	///
	/// Used by rename controllers to correlate async responses. The returned
	/// token is stored so that stale completions can be detected on apply.
	#[cfg(feature = "lsp")]
	fn mint_rename_token(&mut self) -> u64;

	/// Clears the pending rename token, invalidating any in-flight result.
	///
	/// Called when the rename overlay is dismissed without committing.
	#[cfg(feature = "lsp")]
	fn clear_pending_rename_token(&mut self);
}

/// Behavioral logic for a modal interaction session.
pub trait OverlayController: Send + Sync {
	/// Stable identifier for the controller kind.
	fn name(&self) -> &'static str;

	/// Stable semantic kind for frontend/runtime policy decisions.
	fn kind(&self) -> OverlayControllerKind {
		OverlayControllerKind::Other(self.name())
	}

	/// Human-readable identity title for virtual-buffer presentation fallbacks.
	fn identity_title(&self) -> &'static str {
		self.kind().default_title()
	}

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

impl OverlayContext for crate::Editor {
	fn buffer(&self, id: ViewId) -> Option<&Buffer> {
		self.state.core.editor.buffers.get_buffer(id)
	}

	fn buffer_mut(&mut self, id: ViewId) -> Option<&mut Buffer> {
		self.state.core.editor.buffers.get_buffer_mut(id)
	}

	fn reset_buffer_content(&mut self, view: ViewId, content: &str) {
		let Some(buffer) = self.state.core.editor.buffers.get_buffer_mut(view) else {
			return;
		};
		buffer.reset_content(content);
		self.state.integration.syntax_manager.reset_syntax(buffer.document_id());
	}

	fn notify(&mut self, notification: Notification) {
		self.notify(notification);
	}

	fn reveal_cursor_in_view(&mut self, view: ViewId) {
		self.reveal_cursor_in_view(view);
	}

	fn request_redraw(&mut self) {
		self.state.core.frame.needs_redraw = true;
	}

	fn queue_invocation(&mut self, request: xeno_registry::actions::DeferredInvocationRequest) {
		self.enqueue_runtime_invocation_request(request, crate::runtime::work_queue::RuntimeWorkSource::Overlay);
	}

	#[cfg(feature = "lsp")]
	fn worker_runtime(&self) -> xeno_worker::WorkerRuntime {
		self.state.async_state.worker_runtime.clone()
	}

	fn finalize_buffer_removal(&mut self, view: ViewId) {
		self.finalize_buffer_removal(view);
	}

	#[cfg(feature = "lsp")]
	fn msg_tx(&self) -> crate::msg::MsgSender {
		self.msg_tx()
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
		self.state.telemetry.command_usage.record(canonical);
	}

	fn command_usage_snapshot(&self) -> crate::completion::CommandUsageSnapshot {
		self.state.telemetry.command_usage.snapshot()
	}

	fn filesystem(&self) -> &crate::filesystem::FsService {
		&self.state.integration.filesystem
	}

	fn filesystem_mut(&mut self) -> &mut crate::filesystem::FsService {
		&mut self.state.integration.filesystem
	}

	#[cfg(feature = "lsp")]
	fn lsp_prepare_position_request(
		&self,
		buffer: &Buffer,
	) -> xeno_lsp::Result<Option<(xeno_lsp::ClientHandle, xeno_lsp::lsp_types::Uri, xeno_lsp::lsp_types::Position)>> {
		self.state.integration.lsp.prepare_position_request(buffer)
	}

	#[cfg(feature = "lsp")]
	fn apply_workspace_edit<'a>(
		&'a mut self,
		edit: xeno_lsp::lsp_types::WorkspaceEdit,
	) -> Pin<Box<dyn Future<Output = Result<(), crate::lsp::workspace_edit::ApplyError>> + 'a>> {
		Box::pin(self.apply_workspace_edit(edit))
	}

	#[cfg(feature = "lsp")]
	fn mint_rename_token(&mut self) -> u64 {
		let token = self.state.async_state.rename_request_token_next;
		self.state.async_state.rename_request_token_next += 1;
		self.state.async_state.pending_rename_token = Some(token);
		token
	}

	#[cfg(feature = "lsp")]
	fn clear_pending_rename_token(&mut self) {
		self.state.async_state.pending_rename_token = None;
	}
}

/// Trait for passive, contextual overlay behaviors.
///
/// Unlike controllers, layers do not steal focus or own dedicated input buffers.
/// They react to editor events and may intercept keys when visible.
pub trait OverlayLayer: Send + Sync {
	fn name(&self) -> &'static str;

	/// Determines if the layer is currently active.
	fn is_visible(&self, ed: &crate::Editor) -> bool;

	/// Optional key interception for visible layers (e.g. Tab/Enter in completion menus).
	fn on_key(&mut self, _ed: &mut crate::Editor, _key: Key) -> bool {
		false
	}

	/// Notifies the layer about editor state changes.
	fn on_event(&mut self, _ed: &mut crate::Editor, _event: &LayerEvent) {}
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

/// Collection of passive overlay layers with key routing and event propagation.
#[derive(Default)]
pub struct OverlayLayers {
	layers: Vec<Box<dyn OverlayLayer>>,
}

impl OverlayLayers {
	/// Adds a new layer to the top of the stack.
	pub fn add(&mut self, layer: Box<dyn OverlayLayer>) {
		tracing::trace!(layer = layer.name(), "overlay layer added");
		self.layers.push(layer);
	}

	/// Routes key events to visible layers in reverse Z-order.
	pub fn handle_key(&mut self, ed: &mut crate::Editor, key: Key) -> bool {
		for layer in self.layers.iter_mut().rev() {
			if layer.is_visible(ed) && layer.on_key(ed, key) {
				return true;
			}
		}
		false
	}

	/// Propagates events to all layers.
	pub fn notify_event(&mut self, ed: &mut crate::Editor, event: LayerEvent) {
		for layer in &mut self.layers {
			layer.on_event(ed, &event);
		}
	}
}

impl OverlayManager {
	pub fn active(&self) -> Option<&ActiveOverlay> {
		self.active.as_ref()
	}

	/// Returns `true` if a modal interaction is currently active.
	pub fn is_open(&self) -> bool {
		self.active.is_some()
	}

	/// Starts a new modal interaction session.
	///
	/// Fails and returns `false` if an interaction is already active.
	pub fn open(&mut self, ed: &mut crate::Editor, mut controller: Box<dyn OverlayController>) -> bool {
		if self.is_open() {
			return false;
		}

		let spec = controller.ui_spec(ed);
		let desired_height = if matches!(controller.kind(), OverlayControllerKind::CommandPalette) || spec.windows.is_empty() {
			1
		} else {
			10
		};
		ed.state.ui.ui.sync_utility_for_modal_overlay(Some(desired_height));

		if let Some(mut session) = OverlayHost::setup_session(ed, &*controller) {
			#[cfg(feature = "lsp")]
			ed.clear_lsp_menu();

			controller.on_open(ed, &mut session);
			self.active = Some(ActiveOverlay { session, controller });
			true
		} else {
			ed.state.ui.ui.sync_utility_for_modal_overlay(None);
			false
		}
	}

	/// Closes the active interaction session with the specified reason.
	pub fn close(&mut self, ed: &mut crate::Editor, reason: CloseReason) {
		if let Some(mut active) = self.active.take() {
			OverlayHost::cleanup_session(ed, &mut *active.controller, active.session, reason);
		}
	}

	/// Commits and terminates the active interaction session.
	pub async fn commit(&mut self, ed: &mut crate::Editor) {
		if let Some(mut active) = self.active.take() {
			active.controller.on_commit(ed, &mut active.session).await;
			OverlayHost::cleanup_session(ed, &mut *active.controller, active.session, CloseReason::Commit);
		}
	}

	/// Routes key events to the active interaction.
	///
	/// Falls back to default host dismissal (Esc -> Cancel) if the controller
	/// does not handle the key.
	pub fn handle_key(&mut self, ed: &mut crate::Editor, key: Key) -> bool {
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
	pub fn on_buffer_edited(&mut self, ed: &mut crate::Editor, view_id: ViewId) {
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
	pub fn refresh_if_kind(&mut self, ed: &mut crate::Editor, kind: OverlayControllerKind) {
		let Some(active) = self.active.as_mut() else {
			return;
		};
		if active.controller.kind() != kind {
			return;
		}

		let text = active.session.input_text(ed);
		active.controller.on_input_changed(ed, &mut active.session, &text);
	}

	/// Called when terminal viewport dimensions change.
	pub fn on_viewport_changed(&mut self, ed: &mut crate::Editor) {
		let Some(mut active) = self.active.take() else {
			return;
		};

		if OverlayHost::reflow_session(ed, &*active.controller, &mut active.session) {
			ed.state.core.frame.needs_redraw = true;
			self.active = Some(active);
			return;
		}

		OverlayHost::cleanup_session(ed, &mut *active.controller, active.session, CloseReason::Forced);
	}
}

#[cfg(test)]
mod tests;
