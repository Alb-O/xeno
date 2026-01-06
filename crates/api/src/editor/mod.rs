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
//! [`FileOpsAccess`]: xeno_core::editor_ctx::FileOpsAccess

/// Action execution result handling.
mod actions;
/// Action dispatch and context setup.
mod actions_exec;
/// Buffer collection management.
mod buffer_manager;
/// Buffer creation operations.
mod buffer_ops;
/// Command queue for deferred execution.
mod command_queue;
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
/// Input handling.
mod input;
/// Split layout management.
mod layout;
/// Editor lifecycle (tick, render).
mod lifecycle;
/// Message and notification display.
mod messaging;
/// Cursor navigation utilities.
mod navigation;
/// Command palette operations.
mod palette;
/// Search state and operations.
mod search;
/// Separator hit detection.
mod separator;
/// Option resolution.
mod options;
/// Split view operations.
mod splits;
/// Theme management.
mod theming;
/// Shared type definitions.
pub mod types;
/// Buffer access and viewport management.
mod views;

use std::collections::HashSet;
use std::path::PathBuf;

pub use buffer_manager::BufferManager;
pub use command_queue::CommandQueue;
pub use focus::{FocusReason, FocusTarget, PanelId};
pub use hook_runtime::HookRuntime;
pub use layout::{LayoutManager, SeparatorHit, SeparatorId};
pub use types::{HistoryEntry, JumpList, JumpLocation, MacroState, Registers};
use xeno_language::LanguageLoader;
use xeno_registry::options::OptionStore;
use xeno_registry::themes::Theme;
use xeno_registry::{
	HookContext, HookEventData, WindowKind, emit_sync_with as emit_hook_sync_with,
};
use xeno_tui::layout::Rect;
use xeno_tui::widgets::menu::MenuState;

pub use self::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};
use crate::buffer::{BufferId, BufferView, Layout};
use crate::editor::extensions::{EXTENSIONS, ExtensionMap, StyleOverlays};
use crate::editor::types::CompletionState;
use crate::menu::{MenuAction, create_menu};
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

	/// Workspace-level registers (yank buffer, etc.).
	pub registers: Registers,

	/// Current theme.
	pub theme: &'static Theme,

	/// Window dimensions.
	pub window_width: Option<u16>,
	/// Window height in rows.
	pub window_height: Option<u16>,

	/// UI manager (panels, dock, etc.).
	pub ui: UiManager,

	/// Whether a redraw is needed.
	pub needs_redraw: bool,

	/// Whether a command requested the editor to quit.
	pending_quit: bool,

	/// Notification system.
	pub notifications: xeno_tui::widgets::notifications::ToastManager,

	/// Last tick timestamp.
	pub last_tick: std::time::SystemTime,

	/// Completion state.
	pub completions: CompletionState,

	/// Extension map (typemap for extension state).
	pub extensions: ExtensionMap,

	/// Language configuration loader.
	pub language_loader: LanguageLoader,

	/// Style overlays for rendering modifications.
	pub style_overlays: StyleOverlays,

	/// Runtime for scheduling async hooks during sync emission.
	pub hook_runtime: HookRuntime,

	/// Buffers with pending content changes for [`HookEvent::BufferChange`].
	dirty_buffers: HashSet<BufferId>,

	/// Views with sticky focus (resist mouse hover focus changes).
	sticky_views: HashSet<BufferView>,

	/// Jump list for `<C-o>` / `<C-i>` navigation.
	pub jump_list: JumpList,

	/// Macro recording and playback state.
	pub macro_state: MacroState,

	/// Queue for deferred command execution from [`ActionResult::Command`].
	pub command_queue: CommandQueue,

	/// Application menu bar state.
	pub menu: MenuState<MenuAction>,

	/// Command palette state.
	pub palette: crate::palette::PaletteState,

	/// Global user configuration options.
	pub global_options: OptionStore,

	/// Per-language option overrides.
	pub language_options: std::collections::HashMap<String, OptionStore>,
}

impl xeno_core::EditorOps for Editor {}

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

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::WindowCreated {
					window_id: xeno_registry::WindowId(window_manager.base_id().0),
					kind: WindowKind::Base,
				},
				None,
			),
			&mut hook_runtime,
		);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = buffer_manager.focused_buffer();

		emit_hook_sync_with(
			&HookContext::new(
				HookEventData::BufferOpen {
					path: hook_path,
					text: buffer.doc().content.slice(..),
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
			registers: Registers::default(),
			theme: xeno_registry::themes::get_theme(xeno_registry::themes::DEFAULT_THEME_ID)
				.unwrap_or(&xeno_registry::themes::DEFAULT_THEME),
			window_width: None,
			window_height: None,
			ui: UiManager::new(),
			needs_redraw: false,
			pending_quit: false,
			notifications: xeno_tui::widgets::notifications::ToastManager::new()
				.max_visible(Some(5))
				.overflow(xeno_tui::widgets::notifications::Overflow::DropOldest),
			last_tick: std::time::SystemTime::now(),
			completions: CompletionState::default(),
			extensions: {
				let mut map = ExtensionMap::new();
				let mut sorted_exts: Vec<_> = EXTENSIONS.iter().collect();
				sorted_exts.sort_by_key(|e| e.priority);
				for ext in sorted_exts {
					(ext.init)(&mut map);
				}
				map
			},
			language_loader,
			style_overlays: StyleOverlays::new(),
			hook_runtime,
			dirty_buffers: HashSet::new(),
			sticky_views: HashSet::new(),
			jump_list: JumpList::default(),
			macro_state: MacroState::default(),
			command_queue: CommandQueue::new(),
			menu: create_menu(),
			palette: crate::palette::PaletteState::default(),
			global_options: OptionStore::new(),
			language_options: std::collections::HashMap::new(),
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
					window_id: xeno_registry::WindowId(id.0),
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
					window_id: xeno_registry::WindowId(id.0),
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
