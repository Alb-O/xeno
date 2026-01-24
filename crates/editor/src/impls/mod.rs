//! Editor core structure and coordination.
//!
//! The [`Editor`] is the central workspace container, managing buffers, layout,
//! extensions, and UI state. Implementation is split across focused modules:
//!
//! - [`buffer_ops`] - Buffer creation and management
//! - [`editing`] - Text modification operations
//! - [`file_ops`] - File save/load (implements [`FileOpsAccess`])
//! - [`focus`] - View focus and navigation
//! - [`lifecycle`] - Tick, startup, and render updates
//! - [`splits`] - Split view management
//! - [`theming`] - Theme and syntax highlighting
//!
//! [`FileOpsAccess`]: xeno_registry::FileOpsAccess

/// Buffer creation operations.
mod buffer_ops;
/// Core editing state.
mod core;
/// Centralized edit executor.
mod edit_executor;
/// Data-oriented edit operation executor.
mod edit_op_executor;
/// Text editing operations.
mod editing;
/// File save and load operations.
mod file_ops;
/// View focus management.
mod focus;
/// Undo/redo history.
mod history;
/// Unified invocation dispatch.
mod invocation;
/// Editor lifecycle (tick, render).
mod lifecycle;
/// Message and notification display.
mod messaging;
/// Cursor navigation utilities.
mod navigation;
/// Option resolution.
mod options;
/// Search state and operations.
mod search;
/// Split view operations.
mod splits;
/// Theme management.
mod theming;
/// Undo host adapter.
mod undo_host;
/// Buffer access and viewport management.
mod views;

pub use core::EditorCore;
use std::path::PathBuf;
use std::sync::Once;

pub use edit_executor::EditExecutor;
pub use focus::{FocusReason, FocusTarget, PanelId};
pub use navigation::Location;
use xeno_registry::options::OPTIONS;
use xeno_registry::themes::THEMES;
use xeno_registry::{
	HookContext, HookEventData, WindowKind, emit_sync_with as emit_hook_sync_with,
};
use xeno_runtime_language::LanguageLoader;
use xeno_tui::layout::Rect;

use crate::buffer::{Layout, ViewId};
pub use crate::command_queue::CommandQueue;
use crate::extensions::{ExtensionMap, StyleOverlays};
pub use crate::hook_runtime::HookRuntime;
pub use crate::layout::{LayoutManager, SeparatorHit, SeparatorId};
#[cfg(feature = "lsp")]
use crate::lsp::CompletionController;
#[cfg(feature = "lsp")]
use crate::lsp::LspUiEvent;
use crate::overlay::OverlayManager;
pub use crate::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};
pub use crate::types::{
	ApplyEditPolicy, Config, EditorUndoGroup, FrameState, Invocation, InvocationPolicy,
	InvocationResult, JumpList, JumpLocation, MacroState, PreparedEdit, Registers, UndoHost,
	UndoManager, ViewSnapshot, Viewport, Workspace,
};
use crate::ui::UiManager;
pub use crate::view_manager::ViewManager;
use crate::window::{BaseWindow, FloatingStyle, WindowId, WindowManager};

static REGISTRY_SUMMARY_ONCE: Once = Once::new();

fn log_registry_summary_once() {
	REGISTRY_SUMMARY_ONCE.call_once(|| {
		let reg = xeno_registry::index::get_registry();
		tracing::info!(
			actions = reg.actions.base.by_id.len(),
			commands = reg.commands.by_id.len(),
			editor_commands = crate::commands::EDITOR_COMMANDS.len(),
			motions = reg.motions.by_id.len(),
			text_objects = reg.text_objects.by_id.len(),
			gutters = xeno_registry::GUTTERS.len(),
			hooks = xeno_registry::HOOKS.len(),
			notifications = xeno_registry::NOTIFICATIONS.len(),
			options = OPTIONS.len(),
			statusline = xeno_registry::STATUSLINE_SEGMENTS.len(),
			themes = THEMES.len(),
			"registry.summary"
		);
	});
}

/// The main editor/workspace structure.
///
/// Contains text buffers and manages workspace-level state including theme, UI,
/// notifications, and extensions. Supports split views for text buffers.
///
/// # View System
///
/// The editor tracks focus via [`ViewId`] (a type alias for [`ViewId`]).
/// The layout tree arranges views in splits:
///
/// ```text
/// ┌────────────────┬────────────────┐
/// │  Text Buffer   │  Text Buffer   │
/// │   (focused)    │                │
/// └────────────────┴────────────────┘
/// ```
///
/// # Creating an Editor
///
/// ```ignore
/// // Open a file
/// let editor = Editor::new(PathBuf::from("src/main.rs")).await?;
///
/// // Create a scratch buffer
/// let editor = Editor::new_scratch();
/// ```
///
/// # Focus and Navigation
///
/// - [`focused_view`] - Current focus (buffer ID)
/// - [`focus_buffer`] - Focus by ID
/// - [`focus_next_view`] / [`focus_prev_view`] - Cycle through views
///
/// [`ViewId`]: crate::buffer::ViewId
/// [`ViewId`]: crate::buffer::ViewId
/// [`focused_view`]: Self::focused_view
/// [`focus_buffer`]: Self::focus_buffer
/// [`focus_next_view`]: Self::focus_next_view
/// [`focus_prev_view`]: Self::focus_prev_view
pub(crate) struct EditorState {
	/// Core editing state: buffers, workspace, undo history.
	///
	/// Contains essential state for text editing operations. UI, layout,
	/// and presentation concerns are kept separate in other Editor fields.
	pub(crate) core: EditorCore,

	/// Window management (base + floating).
	pub(crate) windows: WindowManager,

	/// Current keyboard focus target.
	pub(crate) focus: focus::FocusTarget,

	/// Layout and split management.
	pub(crate) layout: LayoutManager,

	/// Terminal viewport dimensions.
	pub(crate) viewport: Viewport,

	/// UI manager (panels, dock, etc.).
	pub(crate) ui: UiManager,

	/// Per-frame runtime state (redraw flags, dirty buffers, etc.).
	pub(crate) frame: FrameState,

	/// Editor configuration (theme, languages, options).
	pub(crate) config: Config,

	/// Notification system.
	pub(crate) notifications: xeno_tui::widgets::notifications::ToastManager,

	/// Extension map (typemap for extension state).
	/// Used for loosely-coupled features that can't be direct dependencies.
	pub(crate) extensions: ExtensionMap,

	/// LSP manager for language server integration.
	#[cfg(feature = "lsp")]
	pub(crate) lsp: crate::lsp::LspManager,
	/// Pending LSP changes for debounced sync.
	#[cfg(feature = "lsp")]
	pub(crate) pending_lsp: crate::lsp::pending::PendingLspState,
	/// Completion controller (debounce/cancel/state).
	#[cfg(feature = "lsp")]
	pub(crate) completion_controller: CompletionController,
	/// Signature help request generation.
	#[cfg(feature = "lsp")]
	pub(crate) signature_help_generation: u64,
	/// Signature help cancellation token.
	#[cfg(feature = "lsp")]
	pub(crate) signature_help_cancel: Option<tokio_util::sync::CancellationToken>,
	/// LSP UI event sender.
	#[cfg(feature = "lsp")]
	pub(crate) lsp_ui_tx: tokio::sync::mpsc::UnboundedSender<LspUiEvent>,
	/// LSP UI event receiver.
	#[cfg(feature = "lsp")]
	pub(crate) lsp_ui_rx: tokio::sync::mpsc::UnboundedReceiver<LspUiEvent>,

	/// Style overlays for rendering modifications.
	pub(crate) style_overlays: StyleOverlays,

	/// Runtime for scheduling async hooks during sync emission.
	pub(crate) hook_runtime: HookRuntime,

	/// Type-erased storage for UI overlays (popups, palette, completions).
	pub(crate) overlays: OverlayManager,

	/// Runtime metrics for observability.
	pub(crate) metrics: std::sync::Arc<crate::metrics::EditorMetrics>,
}

pub struct Editor {
	pub(crate) state: EditorState,
}

impl xeno_registry::EditorOps for Editor {}

impl Editor {
	/// Creates a new editor by loading content from the given file path.
	///
	/// If the file exists but is not writable, the buffer is opened in readonly mode.
	pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
		let content = match tokio::fs::read_to_string(&path).await {
			Ok(s) => s,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
			Err(e) => return Err(e.into()),
		};

		let editor = Self::from_content(content, Some(path.clone()));

		if path.exists() && !is_writable(&path) {
			editor.buffer().set_readonly(true);
		}

		Ok(editor)
	}

	/// Creates a new scratch editor with no file association.
	pub fn new_scratch() -> Self {
		Self::from_content(String::new(), None)
	}

	/// Creates an editor from the given content and optional file path.
	pub fn from_content(content: String, path: Option<PathBuf>) -> Self {
		crate::editor_ctx::register_result_handlers();
		log_registry_summary_once();

		// Initialize language loader from embedded languages.kdl
		let language_loader = LanguageLoader::from_embedded();

		// Create buffer manager with initial buffer
		let view_manager = ViewManager::new(content, path.clone(), &language_loader);
		let buffer_id = view_manager.focused_buffer_id().unwrap();
		let window_manager = WindowManager::new(Layout::text(buffer_id), buffer_id);
		let focus = focus::FocusTarget::Buffer {
			window: window_manager.base_id(),
			buffer: buffer_id,
		};

		let mut hook_runtime = HookRuntime::new();

		#[cfg(feature = "lsp")]
		let (lsp_ui_tx, lsp_ui_rx) = tokio::sync::mpsc::unbounded_channel();

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::WindowCreated {
					window_id: window_manager.base_id().into(),
					kind: WindowKind::Base,
				},
				None,
			),
			&mut hook_runtime,
		);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = view_manager.focused_buffer();
		let content = buffer.with_doc(|doc| doc.content().clone());

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::BufferOpen {
					path: hook_path,
					text: content.slice(..),
					file_type: buffer.file_type().as_deref(),
				},
				None,
			),
			&mut hook_runtime,
		);

		// Create EditorCore with buffers, workspace, and undo manager
		let core = EditorCore::new(view_manager, Workspace::default(), UndoManager::new());

		Self {
			state: EditorState {
				core,
				windows: window_manager,
				focus,
				layout: LayoutManager::new(),
				viewport: Viewport::default(),
				ui: UiManager::new(),
				frame: FrameState::default(),
				config: Config::new(language_loader),
				notifications: xeno_tui::widgets::notifications::ToastManager::new()
					.max_visible(Some(5))
					.overflow(xeno_tui::widgets::notifications::Overflow::DropOldest),
				extensions: ExtensionMap::new(),
				#[cfg(feature = "lsp")]
				lsp: crate::lsp::LspManager::new(),
				#[cfg(feature = "lsp")]
				pending_lsp: crate::lsp::pending::PendingLspState::new(),
				#[cfg(feature = "lsp")]
				completion_controller: CompletionController::new(),
				#[cfg(feature = "lsp")]
				signature_help_generation: 0,
				#[cfg(feature = "lsp")]
				signature_help_cancel: None,
				#[cfg(feature = "lsp")]
				lsp_ui_tx,
				#[cfg(feature = "lsp")]
				lsp_ui_rx,
				style_overlays: StyleOverlays::new(),
				hook_runtime,
				overlays: OverlayManager::new(),
				metrics: std::sync::Arc::new(crate::metrics::EditorMetrics::new()),
			},
		}
	}

	/// Returns the base window.
	pub fn base_window(&self) -> &BaseWindow {
		self.state.windows.base_window()
	}

	/// Returns the base window mutably.
	pub fn base_window_mut(&mut self) -> &mut BaseWindow {
		self.state.windows.base_window_mut()
	}

	/// Creates a floating window and emits a hook.
	pub fn create_floating_window(
		&mut self,
		buffer: ViewId,
		rect: Rect,
		style: FloatingStyle,
	) -> WindowId {
		let id = self.state.windows.create_floating(buffer, rect, style);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::WindowCreated {
					window_id: id.into(),
					kind: WindowKind::Floating,
				},
				Some(&self.state.extensions),
			),
			&mut self.state.hook_runtime,
		);
		id
	}

	/// Closes a floating window and emits a hook.
	pub fn close_floating_window(&mut self, id: WindowId) {
		if !matches!(
			self.state.windows.get(id),
			Some(crate::window::Window::Floating(_))
		) {
			return;
		}

		self.state.windows.close_floating(id);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::WindowClosed {
					window_id: id.into(),
				},
				Some(&self.state.extensions),
			),
			&mut self.state.hook_runtime,
		);
	}

	/// Returns a reference to the buffer manager.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.buffers` directly. New code should use `editor.core().buffers`.
	#[inline]
	pub fn buffers(&self) -> &ViewManager {
		&self.state.core.buffers
	}

	/// Returns a mutable reference to the buffer manager.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.buffers` directly. New code should use `editor.core().buffers`.
	#[inline]
	pub fn buffers_mut(&mut self) -> &mut ViewManager {
		&mut self.state.core.buffers
	}

	/// Returns a reference to the workspace session state.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.workspace` directly. New code should use `editor.core().workspace`.
	#[inline]
	pub fn workspace(&self) -> &Workspace {
		&self.state.core.workspace
	}

	/// Returns a mutable reference to the workspace session state.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.workspace` directly. New code should use `editor.core().workspace`.
	#[inline]
	pub fn workspace_mut(&mut self) -> &mut Workspace {
		&mut self.state.core.workspace
	}

	/// Returns a reference to the undo manager.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.undo_manager` directly. New code should use `editor.core().undo_manager`.
	#[inline]
	pub fn undo_manager(&self) -> &UndoManager {
		&self.state.core.undo_manager
	}

	/// Returns a mutable reference to the undo manager.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.undo_manager` directly. New code should use `editor.core().undo_manager`.
	#[inline]
	pub fn undo_manager_mut(&mut self) -> &mut UndoManager {
		&mut self.state.core.undo_manager
	}

	#[inline]
	pub fn core(&self) -> &EditorCore {
		&self.state.core
	}

	#[inline]
	pub fn core_mut(&mut self) -> &mut EditorCore {
		&mut self.state.core
	}

	#[inline]
	pub fn windows(&self) -> &WindowManager {
		&self.state.windows
	}

	#[inline]
	pub fn windows_mut(&mut self) -> &mut WindowManager {
		&mut self.state.windows
	}

	#[inline]
	pub fn focus(&self) -> &FocusTarget {
		&self.state.focus
	}

	#[inline]
	pub fn focus_mut(&mut self) -> &mut FocusTarget {
		&mut self.state.focus
	}

	#[inline]
	pub fn layout(&self) -> &LayoutManager {
		&self.state.layout
	}

	#[inline]
	pub fn layout_mut(&mut self) -> &mut LayoutManager {
		&mut self.state.layout
	}

	#[inline]
	pub fn viewport(&self) -> &Viewport {
		&self.state.viewport
	}

	#[inline]
	pub fn viewport_mut(&mut self) -> &mut Viewport {
		&mut self.state.viewport
	}

	#[inline]
	pub fn ui(&self) -> &UiManager {
		&self.state.ui
	}

	#[inline]
	pub fn ui_mut(&mut self) -> &mut UiManager {
		&mut self.state.ui
	}

	#[inline]
	pub fn frame(&self) -> &FrameState {
		&self.state.frame
	}

	#[inline]
	pub fn frame_mut(&mut self) -> &mut FrameState {
		&mut self.state.frame
	}

	#[inline]
	pub fn config(&self) -> &Config {
		&self.state.config
	}

	#[inline]
	pub fn config_mut(&mut self) -> &mut Config {
		&mut self.state.config
	}

	#[inline]
	pub fn notifications(&self) -> &xeno_tui::widgets::notifications::ToastManager {
		&self.state.notifications
	}

	#[inline]
	pub fn notifications_mut(&mut self) -> &mut xeno_tui::widgets::notifications::ToastManager {
		&mut self.state.notifications
	}

	#[inline]
	pub fn extensions(&self) -> &ExtensionMap {
		&self.state.extensions
	}

	#[inline]
	pub fn extensions_and_hook_runtime_mut(&mut self) -> (&ExtensionMap, &mut HookRuntime) {
		(&self.state.extensions, &mut self.state.hook_runtime)
	}

	#[inline]
	pub fn extensions_mut(&mut self) -> &mut ExtensionMap {
		&mut self.state.extensions
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn lsp(&self) -> &crate::lsp::LspManager {
		&self.state.lsp
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn lsp_mut(&mut self) -> &mut crate::lsp::LspManager {
		&mut self.state.lsp
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn pending_lsp(&self) -> &crate::lsp::pending::PendingLspState {
		&self.state.pending_lsp
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn pending_lsp_mut(&mut self) -> &mut crate::lsp::pending::PendingLspState {
		&mut self.state.pending_lsp
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn completion_controller(&self) -> &CompletionController {
		&self.state.completion_controller
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn completion_controller_mut(&mut self) -> &mut CompletionController {
		&mut self.state.completion_controller
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn signature_help_generation(&self) -> u64 {
		self.state.signature_help_generation
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn signature_help_generation_mut(&mut self) -> &mut u64 {
		&mut self.state.signature_help_generation
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn signature_help_cancel(&self) -> &Option<tokio_util::sync::CancellationToken> {
		&self.state.signature_help_cancel
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn signature_help_cancel_mut(
		&mut self,
	) -> &mut Option<tokio_util::sync::CancellationToken> {
		&mut self.state.signature_help_cancel
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn lsp_ui_tx(&self) -> &tokio::sync::mpsc::UnboundedSender<LspUiEvent> {
		&self.state.lsp_ui_tx
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn lsp_ui_tx_mut(&mut self) -> &mut tokio::sync::mpsc::UnboundedSender<LspUiEvent> {
		&mut self.state.lsp_ui_tx
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn lsp_ui_rx(&self) -> &tokio::sync::mpsc::UnboundedReceiver<LspUiEvent> {
		&self.state.lsp_ui_rx
	}

	#[cfg(feature = "lsp")]
	#[inline]
	pub fn lsp_ui_rx_mut(&mut self) -> &mut tokio::sync::mpsc::UnboundedReceiver<LspUiEvent> {
		&mut self.state.lsp_ui_rx
	}

	#[inline]
	pub fn style_overlays(&self) -> &StyleOverlays {
		&self.state.style_overlays
	}

	#[inline]
	pub fn style_overlays_mut(&mut self) -> &mut StyleOverlays {
		&mut self.state.style_overlays
	}

	#[inline]
	pub fn hook_runtime(&self) -> &HookRuntime {
		&self.state.hook_runtime
	}

	#[inline]
	pub fn hook_runtime_mut(&mut self) -> &mut HookRuntime {
		&mut self.state.hook_runtime
	}

	#[inline]
	pub fn overlays(&self) -> &OverlayManager {
		&self.state.overlays
	}

	#[inline]
	pub fn overlays_mut(&mut self) -> &mut OverlayManager {
		&mut self.state.overlays
	}

	#[inline]
	pub fn metrics(&self) -> &std::sync::Arc<crate::metrics::EditorMetrics> {
		&self.state.metrics
	}

	#[inline]
	pub fn metrics_mut(&mut self) -> &mut std::sync::Arc<crate::metrics::EditorMetrics> {
		&mut self.state.metrics
	}
}

/// Checks if a file is writable by attempting to open it for writing.
fn is_writable(path: &std::path::Path) -> bool {
	std::fs::OpenOptions::new().write(true).open(path).is_ok()
}
