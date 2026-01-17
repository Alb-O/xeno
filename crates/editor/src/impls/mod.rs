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

pub use edit_executor::EditExecutor;
pub use focus::{FocusReason, FocusTarget, PanelId};
pub use navigation::Location;
use xeno_registry::{
	HookContext, HookEventData, WindowKind, emit_sync_with as emit_hook_sync_with,
};
use xeno_runtime_language::LanguageLoader;
use xeno_tui::layout::Rect;

use crate::buffer::{BufferId, Layout};
pub use crate::buffer_manager::BufferManager;
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
use crate::window::{BaseWindow, FloatingStyle, WindowId, WindowManager};

/// The main editor/workspace structure.
///
/// Contains text buffers and manages workspace-level state including theme, UI,
/// notifications, and extensions. Supports split views for text buffers.
///
/// # View System
///
/// The editor tracks focus via [`BufferView`] (a type alias for [`BufferId`]).
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
/// [`BufferView`]: crate::buffer::BufferView
/// [`BufferId`]: crate::buffer::BufferId
/// [`focused_view`]: Self::focused_view
/// [`focus_buffer`]: Self::focus_buffer
/// [`focus_next_view`]: Self::focus_next_view
/// [`focus_prev_view`]: Self::focus_prev_view
pub struct Editor {
	/// Core editing state: buffers, workspace, undo history.
	///
	/// Contains essential state for text editing operations. UI, layout,
	/// and presentation concerns are kept separate in other Editor fields.
	pub core: EditorCore,

	/// Window management (base + floating).
	pub windows: WindowManager,

	/// Current keyboard focus target.
	pub focus: focus::FocusTarget,

	/// Layout and split management.
	pub layout: LayoutManager,

	/// Terminal viewport dimensions.
	pub viewport: Viewport,

	/// UI manager (panels, dock, etc.).
	pub ui: UiManager,

	/// Per-frame runtime state (redraw flags, dirty buffers, etc.).
	pub frame: FrameState,

	/// Editor configuration (theme, languages, options).
	pub config: Config,

	/// Notification system.
	pub notifications: xeno_tui::widgets::notifications::ToastManager,

	/// Extension map (typemap for extension state).
	/// Used for loosely-coupled features that can't be direct dependencies.
	pub extensions: ExtensionMap,

	/// LSP manager for language server integration.
	#[cfg(feature = "lsp")]
	pub lsp: crate::lsp::LspManager,
	/// Pending LSP changes for debounced sync.
	#[cfg(feature = "lsp")]
	pub pending_lsp: crate::lsp::pending::PendingLspState,
	/// Completion controller (debounce/cancel/state).
	#[cfg(feature = "lsp")]
	pub completion_controller: CompletionController,
	/// Signature help request generation.
	#[cfg(feature = "lsp")]
	pub signature_help_generation: u64,
	/// Signature help cancellation token.
	#[cfg(feature = "lsp")]
	pub signature_help_cancel: Option<tokio_util::sync::CancellationToken>,
	/// LSP UI event sender.
	#[cfg(feature = "lsp")]
	pub lsp_ui_tx: tokio::sync::mpsc::UnboundedSender<LspUiEvent>,
	/// LSP UI event receiver.
	#[cfg(feature = "lsp")]
	pub lsp_ui_rx: tokio::sync::mpsc::UnboundedReceiver<LspUiEvent>,

	/// Style overlays for rendering modifications.
	pub style_overlays: StyleOverlays,

	/// Runtime for scheduling async hooks during sync emission.
	pub hook_runtime: HookRuntime,

	/// Type-erased storage for UI overlays (popups, palette, completions).
	pub overlays: OverlayManager,
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
		// Initialize language loader from embedded languages.kdl
		let language_loader = LanguageLoader::from_embedded();

		// Create buffer manager with initial buffer
		let buffer_manager = BufferManager::new(content, path.clone(), &language_loader);
		let buffer_id = buffer_manager.focused_buffer_id().unwrap();
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
		let buffer = buffer_manager.focused_buffer();
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
		let core = EditorCore::new(buffer_manager, Workspace::default(), UndoManager::new());

		Self {
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
		}
	}

	/// Returns the base window.
	pub fn base_window(&self) -> &BaseWindow {
		self.windows.base_window()
	}

	/// Returns the base window mutably.
	pub fn base_window_mut(&mut self) -> &mut BaseWindow {
		self.windows.base_window_mut()
	}

	/// Creates a floating window and emits a hook.
	pub fn create_floating_window(
		&mut self,
		buffer: BufferId,
		rect: Rect,
		style: FloatingStyle,
	) -> WindowId {
		let id = self.windows.create_floating(buffer, rect, style);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::WindowCreated {
					window_id: id.into(),
					kind: WindowKind::Floating,
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);
		id
	}

	/// Closes a floating window and emits a hook.
	pub fn close_floating_window(&mut self, id: WindowId) {
		if !matches!(
			self.windows.get(id),
			Some(crate::window::Window::Floating(_))
		) {
			return;
		}

		self.windows.close_floating(id);
		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::WindowClosed {
					window_id: id.into(),
				},
				Some(&self.extensions),
			),
			&mut self.hook_runtime,
		);
	}

	/// Returns a reference to the buffer manager.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.buffers` directly. New code should use `editor.core.buffers`.
	#[inline]
	pub fn buffers(&self) -> &BufferManager {
		&self.core.buffers
	}

	/// Returns a mutable reference to the buffer manager.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.buffers` directly. New code should use `editor.core.buffers`.
	#[inline]
	pub fn buffers_mut(&mut self) -> &mut BufferManager {
		&mut self.core.buffers
	}

	/// Returns a reference to the workspace session state.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.workspace` directly. New code should use `editor.core.workspace`.
	#[inline]
	pub fn workspace(&self) -> &Workspace {
		&self.core.workspace
	}

	/// Returns a mutable reference to the workspace session state.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.workspace` directly. New code should use `editor.core.workspace`.
	#[inline]
	pub fn workspace_mut(&mut self) -> &mut Workspace {
		&mut self.core.workspace
	}

	/// Returns a reference to the undo manager.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.undo_manager` directly. New code should use `editor.core.undo_manager`.
	#[inline]
	pub fn undo_manager(&self) -> &UndoManager {
		&self.core.undo_manager
	}

	/// Returns a mutable reference to the undo manager.
	///
	/// This is a compatibility accessor for code that previously accessed
	/// `editor.undo_manager` directly. New code should use `editor.core.undo_manager`.
	#[inline]
	pub fn undo_manager_mut(&mut self) -> &mut UndoManager {
		&mut self.core.undo_manager
	}
}

/// Checks if a file is writable by attempting to open it for writing.
fn is_writable(path: &std::path::Path) -> bool {
	std::fs::OpenOptions::new().write(true).open(path).is_ok()
}
