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
/// Editor construction and top-level integration accessors.
mod surface;
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
use xeno_registry::hooks::{HookContext, WindowKind, emit as emit_hook, emit_sync_with as emit_hook_sync_with};
use xeno_registry::options::OPTIONS;
use xeno_registry::themes::THEMES;
use xeno_registry::{HookEventData, KeymapSnapshot};

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

pub(crate) struct CoreStateBundle {
	/// Core editing state: buffers, workspace, undo history.
	pub(crate) editor: EditorCore,
	/// Window management (base).
	pub(crate) windows: WindowManager,
	/// Current keyboard focus target.
	pub(crate) focus: focus::FocusTarget,
	/// Focus epoch - incremented on every focus or structural change.
	pub(crate) focus_epoch: focus::FocusEpoch,
	/// Layout and split management.
	pub(crate) layout: LayoutManager,
	/// Terminal viewport dimensions.
	pub(crate) viewport: Viewport,
	/// Per-frame runtime state (redraw flags, dirty buffers, etc.).
	pub(crate) frame: FrameState,
}

impl std::ops::Deref for CoreStateBundle {
	type Target = EditorCore;

	fn deref(&self) -> &Self::Target {
		&self.editor
	}
}

impl std::ops::DerefMut for CoreStateBundle {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.editor
	}
}

pub(crate) struct RuntimeStateBundle {
	/// Runtime-owned deferred work queue drained by runtime pump phases.
	runtime_work_queue: RuntimeWorkQueue,
	/// Runtime event coordinator queues and directive buffer.
	runtime_kernel: RuntimeKernel,
	/// Active runtime cause propagated while draining one causal chain.
	runtime_active_cause_id: Option<RuntimeCauseId>,
	/// Unified side-effect routing and sink.
	pub(crate) effects: crate::effects::sink::EffectSink,
	/// Recursion depth for side-effect flushing.
	pub(crate) flush_depth: usize,
	/// Session recorder for replay-based integration testing.
	pub(crate) recorder: Option<crate::runtime::recorder::EventRecorder>,
}

pub(crate) struct IntegrationStateBundle {
	/// Nu runtime/executor lifecycle and hook/macro orchestration state.
	pub(crate) nu: crate::nu::coordinator::NuCoordinatorState,
	/// LSP system (real or no-op depending on feature flags).
	pub(crate) lsp: LspSystem,
	/// Background syntax loading manager.
	pub(crate) syntax_manager: xeno_syntax::SyntaxManager,
	/// Unified async work scheduler (hooks, LSP, indexing, watchers).
	pub(crate) work_scheduler: WorkScheduler,
	/// Background filesystem indexing and picker state.
	pub(crate) filesystem: crate::filesystem::FsService,
}

pub(crate) struct UiStateBundle {
	/// UI manager (panels, dock, etc.).
	pub(crate) ui: UiManager,
	/// Unified overlay system for modal interactions and passive layers.
	pub(crate) overlay_system: OverlaySystem,
	/// Notification system.
	pub(crate) notifications: crate::notifications::NotificationCenter,
	/// Render cache for efficient viewport rendering.
	pub(crate) render_cache: crate::render::cache::RenderCache,
}

pub(crate) struct ConfigStateBundle {
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
	/// Whether the asynchronous LSP catalog load has been applied.
	pub(crate) lsp_catalog_ready: bool,
}

impl std::ops::Deref for ConfigStateBundle {
	type Target = Config;

	fn deref(&self) -> &Self::Target {
		&self.config
	}
}

impl std::ops::DerefMut for ConfigStateBundle {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.config
	}
}

pub(crate) struct AsyncStateBundle {
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
}

pub(crate) struct TelemetryStateBundle {
	/// Runtime metrics for observability.
	pub(crate) metrics: std::sync::Arc<crate::metrics::EditorMetrics>,
	/// Command usage tracking for command palette ranking.
	pub(crate) command_usage: crate::completion::CommandPaletteUsage,
}

pub(crate) struct EditorState {
	pub(crate) core: CoreStateBundle,
	pub(crate) runtime: RuntimeStateBundle,
	pub(crate) integration: IntegrationStateBundle,
	pub(crate) ui: UiStateBundle,
	pub(crate) config: ConfigStateBundle,
	pub(crate) async_state: AsyncStateBundle,
	pub(crate) telemetry: TelemetryStateBundle,
}

pub struct Editor {
	pub(crate) state: EditorState,
}

impl EditorState {
	#[inline]
	pub(crate) fn runtime_work_queue(&self) -> &RuntimeWorkQueue {
		&self.runtime.runtime_work_queue
	}

	#[inline]
	pub(crate) fn runtime_work_queue_mut(&mut self) -> &mut RuntimeWorkQueue {
		&mut self.runtime.runtime_work_queue
	}

	#[inline]
	pub(crate) fn runtime_kernel(&self) -> &RuntimeKernel {
		&self.runtime.runtime_kernel
	}

	#[inline]
	pub(crate) fn runtime_kernel_mut(&mut self) -> &mut RuntimeKernel {
		&mut self.runtime.runtime_kernel
	}

	#[inline]
	pub(crate) fn runtime_active_cause_id(&self) -> Option<RuntimeCauseId> {
		self.runtime.runtime_active_cause_id
	}

	#[inline]
	pub(crate) fn set_runtime_active_cause_id(&mut self, cause_id: Option<RuntimeCauseId>) {
		self.runtime.runtime_active_cause_id = cause_id;
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
