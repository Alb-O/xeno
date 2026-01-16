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

/// Action dispatch and context setup.
mod actions_exec;
/// Buffer collection management.
mod buffer_manager;
/// Buffer creation operations.
mod buffer_ops;
/// Command queue for deferred execution.
mod command_queue;
/// LSP completion controller.
#[cfg(feature = "lsp")]
mod completion_controller;
/// Fuzzy filtering for LSP completions.
#[cfg(feature = "lsp")]
mod completion_filter;
/// Data-oriented edit operation executor.
mod edit_op_executor;
/// Text editing operations.
mod editing;
/// Extension container and lifecycle.
pub mod extensions;
/// File save and load operations.
mod file_ops;
/// View focus management.
mod focus;
/// Undo/redo history.
mod history;
/// Async hook execution runtime.
mod hook_runtime;
/// Info popup operations.
mod info_popup;
/// Input handling.
mod input;
/// Split layout management.
mod layout;
/// Editor lifecycle (tick, render).
mod lifecycle;
/// LSP diagnostic navigation helpers.
#[cfg(feature = "lsp")]
mod lsp_diagnostics;
/// LSP UI event handling.
#[cfg(feature = "lsp")]
mod lsp_events;
/// LSP completion and menu handling.
#[cfg(feature = "lsp")]
mod lsp_menu;
/// Message and notification display.
mod messaging;
/// Cursor navigation utilities.
mod navigation;
/// Option resolution.
mod options;
/// Command palette operations.
mod palette;
/// Prompt overlay operations.
#[cfg(feature = "lsp")]
mod prompt;
/// Search state and operations.
mod search;
/// Separator hit detection.
mod separator;
/// Snippet parsing for LSP completions.
#[cfg(feature = "lsp")]
mod snippet;
/// Split view operations.
mod splits;
/// Theme management.
mod theming;
/// Shared type definitions.
pub mod types;
/// Buffer access and viewport management.
mod views;
/// LSP workspace edit planning and apply.
#[cfg(feature = "lsp")]
mod workspace_edit;

use std::path::PathBuf;

pub use buffer_manager::BufferManager;
pub use command_queue::CommandQueue;
pub use focus::{FocusReason, FocusTarget, PanelId};
pub use hook_runtime::HookRuntime;
pub use layout::{LayoutManager, SeparatorHit, SeparatorId};
pub use navigation::Location;
pub use types::{
	Config, EditorUndoEntry, FrameState, HistoryEntry, JumpList, JumpLocation, MacroState,
	Registers, Viewport, Workspace,
};
use xeno_registry::{
	HookContext, HookEventData, WindowKind, emit_sync_with as emit_hook_sync_with,
};
use xeno_runtime_language::LanguageLoader;
use xeno_tui::layout::Rect;

#[cfg(feature = "lsp")]
use self::completion_controller::CompletionController;
#[cfg(feature = "lsp")]
use self::lsp_events::LspUiEvent;
pub use self::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};
use crate::buffer::{BufferId, Layout};
use crate::editor::extensions::{ExtensionMap, StyleOverlays};
use crate::overlay::OverlayManager;
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
	/// Buffer and terminal management.
	pub buffers: BufferManager,

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

	/// Workspace session state (registers, jumps, macros, command queue).
	pub workspace: Workspace,

	/// Editor-level undo grouping stack.
	pub undo_group_stack: Vec<EditorUndoEntry>,
	/// Editor-level redo grouping stack.
	pub redo_group_stack: Vec<EditorUndoEntry>,

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

		Self {
			buffers: buffer_manager,
			windows: window_manager,
			focus,
			layout: LayoutManager::new(),
			viewport: Viewport::default(),
			ui: UiManager::new(),
			frame: FrameState::default(),
			workspace: Workspace::default(),
			undo_group_stack: Vec::new(),
			redo_group_stack: Vec::new(),
			config: Config::new(language_loader),
			notifications: xeno_tui::widgets::notifications::ToastManager::new()
				.max_visible(Some(5))
				.overflow(xeno_tui::widgets::notifications::Overflow::DropOldest),
			extensions: ExtensionMap::new(),
			#[cfg(feature = "lsp")]
			lsp: crate::lsp::LspManager::new(),
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
}

/// Checks if a file is writable by attempting to open it for writing.
fn is_writable(path: &std::path::Path) -> bool {
	std::fs::OpenOptions::new().write(true).open(path).is_ok()
}
