//! Editor core structure and coordination.
//!
//! The [`Editor`] is the central workspace container, managing buffers, layout,
//! and UI state. Implementation is split across focused modules:
//!
//! * `buffer_ops` - Buffer creation and management
//! * `editing` - Text modification operations
//! * `file_ops` - File save/load (implements [`xeno_registry::actions::FileOpsAccess`])
//! * `focus` - View focus and navigation
//! * `lifecycle` - Tick, startup, and render updates
//! * `splits` - Split view management
//! * `theming` - Theme and syntax highlighting
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
pub(crate) mod invocation;
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

use core::EditorCore;
use std::path::PathBuf;
use std::sync::{Arc, Once};

pub use focus::{FocusReason, FocusTarget, PanelId};
pub use navigation::Location;
use parking_lot::Mutex;
use xeno_language::LanguageLoader;
use xeno_registry::HookEventData;
use xeno_registry::db::keymap_registry::KeymapSnapshot;
use xeno_registry::hooks::{HookContext, WindowKind, emit as emit_hook, emit_sync_with as emit_hook_sync_with};
use xeno_registry::options::OPTIONS;
use xeno_registry::themes::THEMES;
use xeno_worker::WorkerRuntime;

use crate::buffer::{Buffer, Layout, ViewId};
use crate::geometry::Rect;
use crate::layout::LayoutManager;
#[cfg(feature = "lsp")]
use crate::lsp::LspHandle;
use crate::lsp::LspSystem;
use crate::msg::{MsgReceiver, MsgSender};
use crate::overlay::{OverlayStore, OverlaySystem};
use crate::paste::normalize_to_lf;
use crate::runtime::RuntimeCauseId;
use crate::runtime::kernel::RuntimeKernel;
use crate::runtime::work_queue::RuntimeWorkQueue;
use crate::scheduler::WorkScheduler;
use crate::types::{Config, FrameState, UndoManager, Viewport, Workspace};
use crate::ui::{PanelRenderTarget, UiManager};
use crate::view_manager::ViewManager;
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
/// * `focused_view` - Current focus (buffer ID)
/// * `focus_buffer` - Focus by ID
/// * `focus_next_view` / `focus_prev_view` - Cycle through views
pub(crate) struct EffectiveKeymapCache {
	pub(crate) catalog_version: u64,
	pub(crate) overrides_hash: u64,
	pub(crate) preset_ptr: usize,
	pub(crate) index: Arc<KeymapSnapshot>,
}

/// Tracks pending background file loads for latest-wins token gating.
///
/// Keyed by path so multiple files can load concurrently without silently
/// dropping earlier completions. Each value is the monotonic token for the
/// latest load request for that path.
pub(crate) type PendingFileLoads = std::collections::HashMap<PathBuf, u64>;

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
	/// Runtime-owned deferred work queue drained by runtime pump phases.
	runtime_work_queue: RuntimeWorkQueue,
	/// Runtime event coordinator queues and directive buffer.
	runtime_kernel: RuntimeKernel,
	/// Active runtime cause propagated while draining one causal chain.
	runtime_active_cause_id: Option<RuntimeCauseId>,
	/// Shared worker runtime root for editor-owned async/background tasks.
	pub(crate) worker_runtime: WorkerRuntime,

	/// Editor configuration (theme, languages, options).
	pub(crate) config: Config,
	/// User keybinding overrides loaded from config files.
	pub(crate) key_overrides: Option<xeno_registry::config::UnresolvedKeys>,
	/// Active keymap preset spec string (e.g., `"vim"`, `"./my.nuon"`).
	pub(crate) keymap_preset_spec: String,
	/// Loaded keymap preset.
	pub(crate) keymap_preset: xeno_registry::keymaps::KeymapPresetRef,
	/// Behavioral flags from the active preset, cached for input dispatch.
	pub(crate) keymap_behavior: xeno_registry::keymaps::KeymapBehavior,
	/// Initial mode for new buffers / preset changes.
	pub(crate) keymap_initial_mode: xeno_primitives::Mode,
	/// Cached effective keymap index for the current catalog version and overrides.
	pub(crate) keymap_cache: Mutex<Option<EffectiveKeymapCache>>,
	/// Nu runtime/executor lifecycle and hook/macro orchestration state.
	pub(crate) nu: crate::nu::coordinator::NuCoordinatorState,

	/// Notification system.
	pub(crate) notifications: crate::notifications::NotificationCenter,

	/// LSP system (real or no-op depending on feature flags).
	pub(crate) lsp: LspSystem,

	/// Background syntax loading manager.
	pub(crate) syntax_manager: xeno_syntax::SyntaxManager,

	/// Unified async work scheduler (hooks, LSP, indexing, watchers).
	pub(crate) work_scheduler: WorkScheduler,

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

	/// Pending background file loads, keyed by path, with latest-wins tokens.
	pub(crate) pending_file_loads: PendingFileLoads,
	/// Monotonic token counter for file load requests.
	pub(crate) file_load_token_next: u64,

	/// Token for the latest theme load request (latest-wins gating).
	pub(crate) pending_theme_load_token: Option<u64>,
	/// Monotonic token counter for theme load requests.
	pub(crate) theme_load_token_next: u64,

	/// Token for the latest LSP catalog load request (latest-wins gating).
	pub(crate) pending_lsp_catalog_load_token: Option<u64>,
	/// Monotonic token counter for LSP catalog load requests.
	#[cfg(feature = "lsp")]
	pub(crate) lsp_catalog_load_token_next: u64,

	/// Token for the in-flight rename request (latest-wins gating).
	#[cfg(feature = "lsp")]
	pub(crate) pending_rename_token: Option<u64>,
	/// Monotonic token counter for rename requests.
	#[cfg(feature = "lsp")]
	pub(crate) rename_request_token_next: u64,

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

	/// Session recorder for replay-based integration testing.
	pub(crate) recorder: Option<crate::runtime::recorder::EventRecorder>,
}

pub struct Editor {
	pub(crate) state: EditorState,
}

impl EditorState {
	#[inline]
	pub(crate) fn runtime_work_queue(&self) -> &RuntimeWorkQueue {
		&self.runtime_work_queue
	}

	#[inline]
	pub(crate) fn runtime_work_queue_mut(&mut self) -> &mut RuntimeWorkQueue {
		&mut self.runtime_work_queue
	}

	#[inline]
	pub(crate) fn runtime_kernel(&self) -> &RuntimeKernel {
		&self.runtime_kernel
	}

	#[inline]
	pub(crate) fn runtime_kernel_mut(&mut self) -> &mut RuntimeKernel {
		&mut self.runtime_kernel
	}

	#[inline]
	pub(crate) fn runtime_active_cause_id(&self) -> Option<RuntimeCauseId> {
		self.runtime_active_cause_id
	}

	#[inline]
	pub(crate) fn set_runtime_active_cause_id(&mut self, cause_id: Option<RuntimeCauseId>) {
		self.runtime_active_cause_id = cause_id;
	}
}

/// Data-only frame planning output for frontend compositors.
///
/// Frontends use this to render the frame without mutating core UI/layout
/// internals directly.
#[derive(Debug, Clone)]
pub struct FrontendFramePlan {
	main_area: Rect,
	status_area: Rect,
	doc_area: Rect,
	panel_render_plan: Vec<PanelRenderTarget>,
}

impl FrontendFramePlan {
	pub fn main_area(&self) -> Rect {
		self.main_area
	}

	pub fn status_area(&self) -> Rect {
		self.status_area
	}

	pub fn doc_area(&self) -> Rect {
		self.doc_area
	}

	pub fn panel_render_plan(&self) -> &[PanelRenderTarget] {
		&self.panel_render_plan
	}
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
		let token = editor.state.file_load_token_next;
		editor.state.file_load_token_next += 1;
		editor.state.pending_file_loads.insert(path.clone(), token);
		editor.kick_file_load(path, token);
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

		// Initialize language loader from embedded languages.nuon
		let language_loader = LanguageLoader::from_embedded();

		// Create buffer manager with initial buffer
		let view_manager = ViewManager::new(content, path.clone(), &language_loader);
		let buffer_id = ViewId(1); // Known initial ID
		let window_manager = WindowManager::new(Layout::text(buffer_id), buffer_id);
		let focus = focus::FocusTarget::Buffer {
			window: window_manager.base_id(),
			buffer: buffer_id,
		};

		let mut work_scheduler = WorkScheduler::new();

		emit_hook_sync_with(
			&HookContext::new(HookEventData::WindowCreated {
				window_id: window_manager.base_id().into(),
				kind: WindowKind::Base,
			}),
			&mut work_scheduler,
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
			&mut work_scheduler,
		);

		// Create EditorCore with buffers, workspace, and undo manager
		let core = EditorCore::new(view_manager, Workspace::default(), UndoManager::new());
		let worker_runtime = WorkerRuntime::new();

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
				runtime_work_queue: RuntimeWorkQueue::default(),
				runtime_kernel: RuntimeKernel::default(),
				runtime_active_cause_id: None,
				worker_runtime: worker_runtime.clone(),
				config: Config::new(language_loader),
				key_overrides: None,
				keymap_preset_spec: xeno_registry::keymaps::DEFAULT_PRESET.to_string(),
				keymap_preset: xeno_registry::keymaps::preset(xeno_registry::keymaps::DEFAULT_PRESET).unwrap_or_else(|| {
					std::sync::Arc::new(xeno_registry::keymaps::KeymapPreset {
						name: std::sync::Arc::from("vim"),
						initial_mode: xeno_primitives::Mode::Normal,
						behavior: xeno_registry::keymaps::KeymapBehavior::default(),
						bindings: Vec::new(),
						prefixes: Vec::new(),
					})
				}),
				keymap_behavior: xeno_registry::keymaps::KeymapBehavior::default(),
				keymap_initial_mode: xeno_primitives::Mode::Normal,
				keymap_cache: Mutex::new(None),
				nu: crate::nu::coordinator::NuCoordinatorState::new_with_runtime(worker_runtime.clone()),
				notifications: crate::notifications::NotificationCenter::new(),
				lsp: LspSystem::new(worker_runtime.clone()),
				syntax_manager: xeno_syntax::SyntaxManager::new_with_runtime(
					xeno_syntax::SyntaxManagerCfg {
						max_concurrency: 2,
						..Default::default()
					},
					worker_runtime.clone(),
				),
				work_scheduler,
				overlay_system: OverlaySystem::default(),
				effects: crate::effects::sink::EffectSink::default(),
				flush_depth: 0,
				metrics: std::sync::Arc::new(crate::metrics::EditorMetrics::new()),
				msg_tx,
				msg_rx,
				pending_file_loads: PendingFileLoads::default(),
				file_load_token_next: 0,
				pending_theme_load_token: None,
				theme_load_token_next: 0,
				pending_lsp_catalog_load_token: None,
				#[cfg(feature = "lsp")]
				lsp_catalog_load_token_next: 0,
				#[cfg(feature = "lsp")]
				pending_rename_token: None,
				#[cfg(feature = "lsp")]
				rename_request_token_next: 0,
				deferred_goto: None,
				lsp_catalog_ready: false,
				render_cache: crate::render::cache::RenderCache::new(),
				command_usage: crate::completion::CommandPaletteUsage::default(),
				filesystem: crate::filesystem::FsService::new_with_runtime(worker_runtime),
				recorder: crate::runtime::recorder::EventRecorder::from_env(),
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

	/// Shuts down filesystem indexing/search actors with a bounded graceful timeout.
	pub async fn shutdown_filesystem(&self) {
		let timeout = std::time::Duration::from_millis(250);
		let report = self.state.filesystem.shutdown(xeno_worker::ActorShutdownMode::Graceful { timeout }).await;
		if report.service.timed_out || report.indexer.timed_out || report.search.timed_out {
			tracing::warn!(
				service_timed_out = report.service.timed_out,
				indexer_timed_out = report.indexer.timed_out,
				search_timed_out = report.search.timed_out,
				"filesystem graceful shutdown timed out; forcing immediate"
			);
			let _ = self.state.filesystem.shutdown(xeno_worker::ActorShutdownMode::Immediate).await;
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

	/// Resolves and applies a keymap preset from a spec string.
	///
	/// The spec can be a builtin name (e.g., `"vim"`), a file path, or a
	/// convention name that resolves to `<config_dir>/keymaps/<name>.nuon`.
	/// On resolution failure, falls back to the default preset and emits a
	/// notification.
	pub fn set_keymap_preset(&mut self, spec: String) {
		let config_dir = crate::paths::get_config_dir();
		self.set_keymap_preset_spec(spec, config_dir.as_deref());
	}

	/// Resolves and applies a keymap preset from a spec string with an explicit
	/// base directory for file resolution.
	pub fn set_keymap_preset_spec(&mut self, spec: String, base_dir: Option<&std::path::Path>) {
		match xeno_registry::keymaps::resolve(&spec, base_dir) {
			Ok(p) => self.apply_preset(p, spec),
			Err(e) => {
				tracing::warn!("failed to resolve preset {spec:?}: {e}");
				let fallback = xeno_registry::keymaps::builtin(xeno_registry::keymaps::DEFAULT_PRESET).expect("default preset must exist");
				self.apply_preset(fallback, xeno_registry::keymaps::DEFAULT_PRESET.to_string());
			}
		}
	}

	fn apply_preset(&mut self, preset: xeno_registry::keymaps::KeymapPresetRef, spec: String) {
		self.state.keymap_behavior = preset.behavior;
		self.state.keymap_initial_mode = preset.initial_mode.clone();
		self.state.keymap_preset = preset;
		self.state.keymap_preset_spec = spec;
		self.state.keymap_cache.lock().take();
		let initial_mode = self.state.keymap_initial_mode.clone();
		self.buffer_mut().input.set_mode(initial_mode.clone());
		self.state.core.buffers.set_initial_mode(initial_mode);
	}

	/// Returns the behavioral flags from the active keymap preset.
	pub fn keymap_behavior(&self) -> xeno_registry::keymaps::KeymapBehavior {
		self.state.keymap_behavior
	}

	/// Returns the initial mode from the active keymap preset.
	pub fn keymap_initial_mode(&self) -> xeno_primitives::Mode {
		self.state.keymap_initial_mode.clone()
	}

	/// Replaces the loaded Nu runtime used by `:nu-run`.
	///
	/// Also creates or destroys the persistent executor thread. Runtime swap
	/// order is:
	/// * drop executor first (old worker receives explicit shutdown)
	/// * update runtime and cached hook decl IDs
	/// * create a fresh executor for the new runtime
	///
	/// This prevents a mixed state where cached IDs belong to a new runtime
	/// while jobs are still executing on an old worker.
	pub fn set_nu_runtime(&mut self, runtime: Option<crate::nu::NuRuntime>) {
		self.state.nu.set_runtime(runtime);
	}

	/// Returns the currently loaded Nu runtime, if any.
	pub fn nu_runtime(&self) -> Option<&crate::nu::NuRuntime> {
		self.state.nu.runtime()
	}

	/// Returns the Nu executor, creating one from the current runtime if the
	/// executor is missing (e.g. after a worker thread panic or first access).
	pub fn ensure_nu_executor(&mut self) -> Option<&crate::nu::executor::NuExecutor> {
		self.state.nu.ensure_executor()
	}

	/// Returns the effective keymap for the current catalog version, preset, and overrides.
	pub fn effective_keymap(&self) -> Arc<KeymapSnapshot> {
		let catalog_version = xeno_registry::CATALOG.version_hash();
		let snap = xeno_registry::db::ACTIONS.snapshot();
		let overrides_hash = hash_unresolved_keys(self.state.key_overrides.as_ref());
		let preset_ptr = Arc::as_ptr(&self.state.keymap_preset) as usize;

		{
			let cache = self.state.keymap_cache.lock();
			if let Some(cache) = cache.as_ref()
				&& cache.catalog_version == catalog_version
				&& cache.overrides_hash == overrides_hash
				&& cache.preset_ptr == preset_ptr
			{
				return Arc::clone(&cache.index);
			}
		}

		let index = Arc::new(KeymapSnapshot::build_with_preset(
			&snap,
			Some(&self.state.keymap_preset),
			self.state.key_overrides.as_ref(),
		));
		let mut cache = self.state.keymap_cache.lock();
		*cache = Some(EffectiveKeymapCache {
			catalog_version,
			overrides_hash,
			preset_ptr,
			index: Arc::clone(&index),
		});
		index
	}

	#[inline]
	pub fn take_notification_render_items(&mut self) -> Vec<crate::notifications::NotificationRenderItem> {
		self.state.notifications.take_pending_render_items()
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
	pub(crate) fn work_scheduler_mut(&mut self) -> &mut WorkScheduler {
		&mut self.state.work_scheduler
	}

	/// Emits the editor-start lifecycle hook on the work scheduler.
	pub fn emit_editor_start_hook(&mut self) {
		emit_hook_sync_with(&HookContext::new(HookEventData::EditorStart), &mut self.state.work_scheduler);
	}

	/// Emits the editor-quit lifecycle hook asynchronously.
	pub async fn emit_editor_quit_hook(&self) {
		emit_hook(&HookContext::new(HookEventData::EditorQuit)).await;
	}

	#[inline]
	pub(crate) fn overlays(&self) -> &OverlayStore {
		self.state.overlay_system.store()
	}

	#[inline]
	pub(crate) fn overlays_mut(&mut self) -> &mut OverlayStore {
		self.state.overlay_system.store_mut()
	}

	/// Returns the active modal controller kind for frontend policy.
	#[inline]
	pub fn overlay_kind(&self) -> Option<crate::overlay::OverlayControllerKind> {
		let active = self.state.overlay_system.interaction().active()?;
		Some(active.controller.kind())
	}

	/// Returns a data-only pane plan for the active modal overlay session.
	#[inline]
	pub fn overlay_pane_render_plan(&self) -> Vec<crate::overlay::OverlayPaneRenderTarget> {
		self.state
			.overlay_system
			.interaction()
			.active()
			.map_or_else(Vec::new, |active| active.session.pane_render_plan())
	}

	/// Returns the active modal pane rect for a role when available.
	#[inline]
	pub fn overlay_pane_rect(&self, role: crate::overlay::WindowRole) -> Option<crate::geometry::Rect> {
		let active = self.state.overlay_system.interaction().active()?;
		active.session.pane_rect(role)
	}

	#[inline]
	pub fn whichkey_desired_height(&self) -> Option<u16> {
		crate::ui::utility_whichkey_desired_height(self)
	}

	#[inline]
	pub fn whichkey_render_plan(&self) -> Option<crate::ui::UtilityWhichKeyPlan> {
		crate::ui::utility_whichkey_render_plan(self)
	}

	#[inline]
	pub fn statusline_render_plan(&self) -> Vec<crate::ui::StatuslineRenderSegment> {
		crate::ui::statusline_render_plan(self)
	}

	#[inline]
	pub fn statusline_segment_style(&self, style: crate::ui::StatuslineRenderStyle) -> xeno_primitives::Style {
		crate::ui::statusline_segment_style(self, style)
	}

	/// Number of grid rows reserved for the statusline.
	#[inline]
	pub fn statusline_rows(&self) -> u16 {
		crate::ui::STATUSLINE_ROWS
	}

	/// Clears the per-frame redraw flag after a frontend completes drawing.
	#[inline]
	pub fn mark_frame_drawn(&mut self) {
		self.state.frame.needs_redraw = false;
	}

	/// Prepares a frontend frame using a backend-neutral viewport.
	///
	/// This centralizes per-frame editor updates that were previously performed in
	/// frontend compositor code (viewport sync, UI dock planning, and separator
	/// hover animation activation).
	pub fn begin_frontend_frame(&mut self, viewport: Rect) -> FrontendFramePlan {
		self.state.frame.needs_redraw = false;
		self.ensure_syntax_for_buffers();
		self.state.viewport.width = Some(viewport.width);
		self.state.viewport.height = Some(viewport.height);

		let status_rows = self.statusline_rows().min(viewport.height);
		let main_rows = viewport.height.saturating_sub(status_rows);
		let main_area = Rect::new(viewport.x, viewport.y, viewport.width, main_rows);
		let status_area = Rect::new(viewport.x, viewport.y.saturating_add(main_rows), viewport.width, status_rows);

		let mut ui = std::mem::take(&mut self.state.ui);
		ui.sync_utility_for_modal_overlay(self.utility_overlay_height_hint());
		ui.sync_utility_for_whichkey(self.whichkey_desired_height());
		let dock_layout = ui.compute_layout(main_area);
		let panel_render_plan = ui.panel_render_plan(&dock_layout);
		let doc_area = dock_layout.doc_area;
		self.state.viewport.doc_area = Some(doc_area);

		let activate_separator_hover = {
			let layout = &self.state.layout;
			layout.hovered_separator.is_none() && layout.separator_under_mouse.is_some() && !layout.is_mouse_fast()
		};
		if activate_separator_hover {
			let layout = &mut self.state.layout;
			let old_hover = layout.hovered_separator.take();
			layout.hovered_separator = layout.separator_under_mouse;
			if old_hover != layout.hovered_separator {
				layout.update_hover_animation(old_hover, layout.hovered_separator);
				self.state.frame.needs_redraw = true;
			}
		}
		if self.state.layout.animation_needs_redraw() {
			self.state.frame.needs_redraw = true;
		}
		if ui.take_wants_redraw() {
			self.state.frame.needs_redraw = true;
		}
		self.state.ui = ui;

		FrontendFramePlan {
			main_area,
			status_area,
			doc_area,
			panel_render_plan,
		}
	}

	/// Returns utility panel height hint while a modal overlay is active.
	///
	/// Frontends use this to keep utility panel sizing policy consistent
	/// without depending on controller/session internals.
	#[inline]
	pub fn utility_overlay_height_hint(&self) -> Option<u16> {
		let kind = self.overlay_kind()?;

		if matches!(
			kind,
			crate::overlay::OverlayControllerKind::CommandPalette | crate::overlay::OverlayControllerKind::FilePicker
		) {
			let menu_rows = self.completion_visible_rows(crate::CompletionState::MAX_VISIBLE) as u16;
			Some((1 + menu_rows).clamp(1, 10))
		} else if self.overlay_pane_render_plan().len() <= 1 {
			Some(1)
		} else {
			Some(10)
		}
	}

	#[inline]
	pub fn syntax_manager(&self) -> &xeno_syntax::SyntaxManager {
		&self.state.syntax_manager
	}

	#[inline]
	pub fn syntax_manager_mut(&mut self) -> &mut xeno_syntax::SyntaxManager {
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
		self.drain_messages_report().dirty
	}

	/// Drains pending messages and reports aggregate dirty flags and progress.
	pub(crate) fn drain_messages_report(&mut self) -> MessageDrainReport {
		let mut report = MessageDrainReport {
			dirty: crate::msg::Dirty::NONE,
			drained_count: 0,
		};
		while let Ok(msg) = self.state.msg_rx.try_recv() {
			report.drained_count += 1;
			report.dirty |= msg.apply(self);
		}
		report
	}
}

/// Report emitted after draining pending async editor messages.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MessageDrainReport {
	pub(crate) dirty: crate::msg::Dirty,
	pub(crate) drained_count: usize,
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
		entries.sort_by(|(key_a, opt_a), (key_b, opt_b)| key_a.cmp(key_b).then_with(|| opt_a.cmp(opt_b)));
		for (key, opt_inv) in entries {
			key.hash(&mut hasher);
			opt_inv.hash(&mut hasher);
		}
	}

	hasher.finish()
}

/// Checks if a file is writable by attempting to open it for writing.
fn is_writable(path: &std::path::Path) -> bool {
	std::fs::OpenOptions::new().write(true).open(path).is_ok()
}
