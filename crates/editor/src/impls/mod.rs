//! Editor core structure and coordination.
//!
//! The [`Editor`] is the central workspace container, managing buffers, layout,
//! and UI state. Implementation is split across focused modules:
//!
//! - `buffer_ops` - Buffer creation and management
//! - `editing` - Text modification operations
//! - `file_ops` - File save/load (implements [`xeno_registry::actions::FileOpsAccess`])
//! - `focus` - View focus and navigation
//! - `lifecycle` - Tick, startup, and render updates
//! - `splits` - Split view management
//! - `theming` - Theme and syntax highlighting
//!

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
/// Interaction manager for active overlays.
mod interaction;
/// Unified invocation dispatch.
mod invocation;
/// Background task spawning helpers.
mod kick;
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
use std::sync::{Arc, Once};

pub use edit_executor::EditExecutor;
pub use focus::{FocusReason, FocusTarget, PanelId};
pub use navigation::Location;
use parking_lot::Mutex;
use xeno_language::LanguageLoader;
use xeno_registry::actions::ActionEntry;
use xeno_registry::core::index::Snapshot;
use xeno_registry::db::keymap_registry::KeymapIndex;
use xeno_registry::hooks::{HookContext, WindowKind, emit_sync_with as emit_hook_sync_with};
use xeno_registry::options::OPTIONS;
use xeno_registry::themes::THEMES;
use xeno_registry::{ActionId, HookEventData};

use crate::buffer::{Buffer, Layout, ViewId};
pub use crate::command_queue::CommandQueue;
pub use crate::hook_runtime::HookRuntime;
pub use crate::layout::{LayoutManager, SeparatorHit, SeparatorId};
#[cfg(feature = "lsp")]
use crate::lsp::LspHandle;
use crate::lsp::LspSystem;
use crate::msg::{MsgReceiver, MsgSender};
pub use crate::overlay::{OverlayStore, OverlaySystem};
use crate::paste::normalize_to_lf;
pub use crate::separator::{DragState, MouseVelocityTracker, SeparatorHoverAnimation};
pub use crate::types::{
	ApplyEditPolicy, Config, EditorUndoGroup, FrameState, Invocation, InvocationPolicy, InvocationResult, JumpList, JumpLocation, MacroState, PreparedEdit,
	Registers, UndoHost, UndoManager, ViewSnapshot, Viewport, Workspace,
};
use crate::ui::UiManager;
pub use crate::view_manager::ViewManager;
use crate::window::{BaseWindow, WindowManager};

static REGISTRY_SUMMARY_ONCE: Once = Once::new();

fn log_registry_summary_once() {
	REGISTRY_SUMMARY_ONCE.call_once(|| {
		tracing::info!(
			actions = xeno_registry::ACTIONS.len(),
			commands = xeno_registry::COMMANDS.len(),
			editor_commands = crate::commands::EDITOR_COMMANDS.len(),
			motions = xeno_registry::MOTIONS.len(),
			text_objects = xeno_registry::TEXT_OBJECTS.len(),
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
/// and notifications. Supports split views for text buffers.
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
/// - `focused_view` - Current focus (buffer ID)
/// - `focus_buffer` - Focus by ID
/// - `focus_next_view` / `focus_prev_view` - Cycle through views
pub(crate) struct EffectiveKeymapCache {
	pub(crate) snap: Arc<Snapshot<ActionEntry, ActionId>>,
	pub(crate) overrides_hash: u64,
	pub(crate) index: Arc<KeymapIndex>,
}

pub(crate) struct EditorState {
	/// Core editing state: buffers, workspace, undo history.
	///
	/// Contains essential state for text editing operations. UI, layout,
	/// and presentation concerns are kept separate in other Editor fields.
	pub(crate) core: EditorCore,

	/// Window management (base).
	pub(crate) windows: WindowManager,

	/// Current keyboard focus target.
	pub(crate) focus: focus::FocusTarget,

	/// Focus epoch - incremented on every focus or structural change.
	///
	/// Used by async tasks to detect stale view references.
	pub(crate) focus_epoch: focus::FocusEpoch,

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
	/// User keybinding overrides loaded from config files.
	pub(crate) key_overrides: Option<xeno_registry::config::UnresolvedKeys>,
	/// Cached effective keymap index for the current actions snapshot and overrides.
	pub(crate) keymap_cache: Mutex<Option<EffectiveKeymapCache>>,
	/// Loaded Nu macro runtime from `xeno.nu`.
	pub(crate) nu_runtime: Option<crate::nu::NuRuntime>,
	/// Prevents Nu hook invocations from recursively triggering more hooks.
	pub(crate) nu_hook_guard: bool,

	/// Notification system.
	pub(crate) notifications: crate::notifications::NotificationCenter,

	/// LSP system (real or no-op depending on feature flags).
	pub(crate) lsp: LspSystem,

	/// Background syntax loading manager.
	pub(crate) syntax_manager: crate::syntax_manager::SyntaxManager,

	/// Runtime for scheduling async hooks during sync emission.
	pub(crate) hook_runtime: HookRuntime,

	/// Unified overlay system for modal interactions and passive layers.
	pub(crate) overlay_system: OverlaySystem,

	/// Unified side-effect routing and sink.
	pub(crate) effects: crate::effects::sink::EffectSink,

	/// Recursion depth for side-effect flushing.
	pub(crate) flush_depth: usize,

	/// Runtime metrics for observability.
	pub(crate) metrics: std::sync::Arc<crate::metrics::EditorMetrics>,

	/// Message sender for background tasks.
	pub(crate) msg_tx: MsgSender,
	/// Message receiver for main loop drain.
	pub(crate) msg_rx: MsgReceiver,

	/// Path of file currently being loaded in background, if any.
	pub(crate) loading_file: Option<PathBuf>,

	/// Deferred cursor position to apply after file loads (line, column).
	pub(crate) deferred_goto: Option<(usize, usize)>,

	/// Whether the asynchronous LSP catalog load has been applied.
	pub(crate) lsp_catalog_ready: bool,

	/// Render cache for efficient viewport rendering.
	pub(crate) render_cache: crate::render::cache::RenderCache,

	/// Command usage tracking for command palette ranking.
	pub(crate) command_usage: crate::completion::CommandPaletteUsage,

	/// Background filesystem indexing and picker state.
	pub(crate) filesystem: crate::filesystem::FsService,
}

pub struct Editor {
	pub(crate) state: EditorState,
}

impl Editor {
	/// Creates an editor with a file path, loading content in the background.
	///
	/// Returns immediately with an empty buffer and loading indicator. Content
	/// is loaded asynchronously via [`kick_file_load`] and swapped in when ready.
	///
	/// [`kick_file_load`]: Self::kick_file_load
	pub fn new_with_path(path: PathBuf) -> Self {
		let mut editor = Self::from_content(String::new(), Some(path.clone()));
		editor.state.loading_file = Some(path.clone());
		editor.kick_file_load(path);
		editor
	}

	/// Sets a deferred goto position to apply after file finishes loading.
	pub fn set_deferred_goto(&mut self, line: usize, column: usize) {
		self.state.deferred_goto = Some((line, column));
	}

	/// Creates a new editor by loading content from the given file path.
	///
	/// Prefer [`new_with_path`] for non-blocking startup. This method blocks
	/// on file I/O before returning.
	///
	/// [`new_with_path`]: Self::new_with_path
	pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
		let content = match tokio::fs::read_to_string(&path).await {
			Ok(s) => normalize_to_lf(s),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
			Err(e) => return Err(e.into()),
		};

		let mut editor = Self::from_content(content, Some(path.clone()));

		if path.exists() && !is_writable(&path) {
			editor.buffer_mut().set_readonly(true);
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

		let (msg_tx, msg_rx) = crate::msg::channel();

		// Initialize language loader from embedded languages.kdl
		let language_loader = LanguageLoader::from_embedded();

		// Create buffer manager with initial buffer
		let view_manager = ViewManager::new(content, path.clone(), &language_loader);
		let buffer_id = ViewId(1); // Known initial ID
		let window_manager = WindowManager::new(Layout::text(buffer_id), buffer_id);
		let focus = focus::FocusTarget::Buffer {
			window: window_manager.base_id(),
			buffer: buffer_id,
		};

		let mut hook_runtime = HookRuntime::new();

		emit_hook_sync_with(
			&HookContext::new(HookEventData::WindowCreated {
				window_id: window_manager.base_id().into(),
				kind: WindowKind::Base,
			}),
			&mut hook_runtime,
		);

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);
		let buffer = view_manager.get_buffer(buffer_id).expect("initial buffer exists");
		let content = buffer.with_doc(|doc| doc.content().clone());

		emit_hook_sync_with(
			&HookContext::new(HookEventData::BufferOpen {
				path: hook_path,
				text: content.slice(..),
				file_type: buffer.file_type().as_deref(),
			}),
			&mut hook_runtime,
		);

		// Create EditorCore with buffers, workspace, and undo manager
		let core = EditorCore::new(view_manager, Workspace::default(), UndoManager::new());

		Self {
			state: EditorState {
				core,
				windows: window_manager,
				focus,
				focus_epoch: focus::FocusEpoch::initial(),
				layout: LayoutManager::new(),
				viewport: Viewport::default(),
				ui: UiManager::new(),
				frame: FrameState::default(),
				config: Config::new(language_loader),
				key_overrides: None,
				keymap_cache: Mutex::new(None),
				nu_runtime: None,
				nu_hook_guard: false,
				notifications: crate::notifications::NotificationCenter::new(),
				lsp: LspSystem::new(),
				syntax_manager: crate::syntax_manager::SyntaxManager::new(crate::syntax_manager::SyntaxManagerCfg {
					max_concurrency: 2,
					..Default::default()
				}),
				hook_runtime,
				overlay_system: OverlaySystem::default(),
				effects: crate::effects::sink::EffectSink::default(),
				flush_depth: 0,
				metrics: std::sync::Arc::new(crate::metrics::EditorMetrics::new()),
				msg_tx,
				msg_rx,
				loading_file: None,
				deferred_goto: None,
				lsp_catalog_ready: false,
				render_cache: crate::render::cache::RenderCache::new(),
				command_usage: crate::completion::CommandPaletteUsage::default(),
				filesystem: crate::filesystem::FsService::new(),
			},
		}
	}

	/// Configure a language server.
	pub fn configure_language_server(&mut self, _language: impl Into<String>, _config: crate::lsp::api::LanguageServerConfig) {
		#[cfg(feature = "lsp")]
		self.state.lsp.configure_server(_language, _config);
	}

	/// Removes a language server configuration.
	pub fn remove_language_server(&mut self, _language: &str) {
		#[cfg(feature = "lsp")]
		self.state.lsp.remove_server(_language);
	}

	/// Returns total error count across all buffers.
	pub fn total_error_count(&self) -> usize {
		#[cfg(feature = "lsp")]
		{
			self.state.lsp.total_error_count()
		}
		#[cfg(not(feature = "lsp"))]
		{
			0
		}
	}

	/// Returns total warning count across all buffers.
	pub fn total_warning_count(&self) -> usize {
		#[cfg(feature = "lsp")]
		{
			self.state.lsp.total_warning_count()
		}
		#[cfg(not(feature = "lsp"))]
		{
			0
		}
	}

	/// Returns error count for the given buffer.
	pub fn error_count(&self, _buffer: &Buffer) -> usize {
		#[cfg(feature = "lsp")]
		{
			self.state.lsp.error_count(_buffer)
		}
		#[cfg(not(feature = "lsp"))]
		{
			0
		}
	}

	/// Returns warning count for the given buffer.
	pub fn warning_count(&self, _buffer: &Buffer) -> usize {
		#[cfg(feature = "lsp")]
		{
			self.state.lsp.warning_count(_buffer)
		}
		#[cfg(not(feature = "lsp"))]
		{
			0
		}
	}

	/// Returns diagnostics for the given buffer.
	pub fn get_diagnostics(&self, _buffer: &Buffer) -> Vec<crate::lsp::api::Diagnostic> {
		#[cfg(feature = "lsp")]
		{
			self.state.lsp.get_diagnostics(_buffer)
		}
		#[cfg(not(feature = "lsp"))]
		{
			Vec::new()
		}
	}

	/// Shuts down all language servers.
	pub async fn shutdown_lsp(&self) {
		#[cfg(feature = "lsp")]
		self.state.lsp.shutdown_all().await;
	}

	/// Returns the base window.
	pub fn base_window(&self) -> &BaseWindow {
		self.state.windows.base_window()
	}

	/// Returns the base window mutably.
	pub fn base_window_mut(&mut self) -> &mut BaseWindow {
		self.state.windows.base_window_mut()
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

	/// Sets keybinding overrides and invalidates the effective keymap cache.
	pub fn set_key_overrides(&mut self, keys: Option<xeno_registry::config::UnresolvedKeys>) {
		self.state.key_overrides = keys;
		self.state.keymap_cache.lock().take();
	}

	/// Replaces the loaded Nu runtime used by `:nu-run`.
	pub fn set_nu_runtime(&mut self, runtime: Option<crate::nu::NuRuntime>) {
		self.state.nu_runtime = runtime;
	}

	/// Returns the currently loaded Nu runtime, if any.
	pub fn nu_runtime(&self) -> Option<&crate::nu::NuRuntime> {
		self.state.nu_runtime.as_ref()
	}

	/// Returns the effective keymap for the current actions snapshot and overrides.
	pub fn effective_keymap(&self) -> Arc<KeymapIndex> {
		let snap = xeno_registry::db::ACTIONS.snapshot();
		let overrides_hash = hash_unresolved_keys(self.state.key_overrides.as_ref());

		{
			let cache = self.state.keymap_cache.lock();
			if let Some(cache) = cache.as_ref()
				&& Arc::ptr_eq(&cache.snap, &snap)
				&& cache.overrides_hash == overrides_hash
			{
				return Arc::clone(&cache.index);
			}
		}

		let index = Arc::new(KeymapIndex::build_with_overrides(&snap, self.state.key_overrides.as_ref()));
		let mut cache = self.state.keymap_cache.lock();
		*cache = Some(EffectiveKeymapCache {
			snap,
			overrides_hash,
			index: Arc::clone(&index),
		});
		index
	}

	#[inline]
	pub fn take_notifications(&mut self) -> Vec<xeno_registry::notifications::Notification> {
		self.state.notifications.take_pending()
	}

	#[inline]
	pub fn notifications_clear_epoch(&self) -> u64 {
		self.state.notifications.clear_epoch()
	}

	#[inline]
	#[cfg_attr(not(feature = "lsp"), allow(dead_code))]
	pub(crate) fn lsp(&self) -> &LspSystem {
		&self.state.lsp
	}

	#[inline]
	#[allow(dead_code)]
	pub(crate) fn lsp_mut(&mut self) -> &mut LspSystem {
		&mut self.state.lsp
	}

	#[inline]
	#[cfg(feature = "lsp")]
	pub(crate) fn lsp_handle(&self) -> LspHandle {
		self.state.lsp.handle()
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
	pub fn overlays(&self) -> &OverlayStore {
		self.state.overlay_system.store()
	}

	#[inline]
	pub fn overlays_mut(&mut self) -> &mut OverlayStore {
		self.state.overlay_system.store_mut()
	}

	#[inline]
	pub fn overlay_interaction(&self) -> &crate::overlay::OverlayManager {
		self.state.overlay_system.interaction()
	}

	#[inline]
	pub fn whichkey_desired_height(&self) -> Option<u16> {
		crate::ui::utility_whichkey_desired_height(self)
	}

	#[inline]
	pub fn syntax_manager(&self) -> &crate::syntax_manager::SyntaxManager {
		&self.state.syntax_manager
	}

	#[inline]
	pub fn syntax_manager_mut(&mut self) -> &mut crate::syntax_manager::SyntaxManager {
		&mut self.state.syntax_manager
	}

	#[inline]
	pub fn render_cache(&self) -> &crate::render::cache::RenderCache {
		&self.state.render_cache
	}

	#[inline]
	pub fn render_cache_mut(&mut self) -> &mut crate::render::cache::RenderCache {
		&mut self.state.render_cache
	}

	#[inline]
	pub fn metrics(&self) -> &std::sync::Arc<crate::metrics::EditorMetrics> {
		&self.state.metrics
	}

	#[inline]
	pub fn metrics_mut(&mut self) -> &mut std::sync::Arc<crate::metrics::EditorMetrics> {
		&mut self.state.metrics
	}

	/// Returns a cloneable message sender for background tasks.
	#[inline]
	pub fn msg_tx(&self) -> MsgSender {
		self.state.msg_tx.clone()
	}

	/// Drains pending messages, applying them to editor state.
	///
	/// Returns aggregated dirty flags indicating what needs redraw.
	pub fn drain_messages(&mut self) -> crate::msg::Dirty {
		let mut dirty = crate::msg::Dirty::NONE;
		while let Ok(msg) = self.state.msg_rx.try_recv() {
			dirty |= msg.apply(self);
		}
		dirty
	}
}

fn hash_unresolved_keys(keys: Option<&xeno_registry::config::UnresolvedKeys>) -> u64 {
	use std::hash::{Hash, Hasher};

	let Some(keys) = keys else {
		return 0;
	};

	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	let mut modes: Vec<_> = keys.modes.iter().collect();
	modes.sort_by(|(a, _), (b, _)| a.cmp(b));

	for (mode, bindings) in modes {
		mode.hash(&mut hasher);
		let mut entries: Vec<_> = bindings.iter().collect();
		entries.sort_by(|(key_a, action_a), (key_b, action_b)| key_a.cmp(key_b).then_with(|| action_a.cmp(action_b)));
		for (key, action) in entries {
			key.hash(&mut hasher);
			action.hash(&mut hasher);
		}
	}

	hasher.finish()
}

/// Checks if a file is writable by attempting to open it for writing.
fn is_writable(path: &std::path::Path) -> bool {
	std::fs::OpenOptions::new().write(true).open(path).is_ok()
}
