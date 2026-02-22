use super::*;

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
		let token = editor.state.async_state.file_load_token_next;
		editor.state.async_state.file_load_token_next += 1;
		editor.state.async_state.pending_file_loads.insert(path.clone(), token);
		editor.kick_file_load(path, token);
		editor
	}

	/// Sets a deferred goto position to apply after file finishes loading.
	pub fn set_deferred_goto(&mut self, line: usize, column: usize) {
		self.state.async_state.deferred_goto = Some((line, column));
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

	#[cfg(all(test, feature = "lsp"))]
	pub fn new_scratch_with_transport(transport: std::sync::Arc<dyn xeno_lsp::client::LspTransport>) -> Self {
		log_registry_summary_once();

		let (msg_tx, msg_rx) = crate::msg::channel();
		let (core, work_scheduler, language_loader) = Self::bootstrap_core(String::new(), None);
		let runtime = Self::bootstrap_runtime();
		let mut integration = Self::bootstrap_integrations(work_scheduler);
		integration.lsp = LspSystem::with_transport(transport);
		let ui = Self::bootstrap_ui();
		let config = Self::bootstrap_config(language_loader);
		let async_state = Self::bootstrap_async(msg_tx, msg_rx);
		let telemetry = Self::bootstrap_telemetry();
		let state = Self::assemble_editor_state(core, runtime, integration, ui, config, async_state, telemetry);

		Self { state }
	}

	/// Creates an editor from the given content and optional file path.
	pub fn from_content(content: String, path: Option<PathBuf>) -> Self {
		log_registry_summary_once();

		let (msg_tx, msg_rx) = crate::msg::channel();
		let (core, work_scheduler, language_loader) = Self::bootstrap_core(content, path);
		let runtime = Self::bootstrap_runtime();
		let integration = Self::bootstrap_integrations(work_scheduler);
		let ui = Self::bootstrap_ui();
		let config = Self::bootstrap_config(language_loader);
		let async_state = Self::bootstrap_async(msg_tx, msg_rx);
		let telemetry = Self::bootstrap_telemetry();
		let state = Self::assemble_editor_state(core, runtime, integration, ui, config, async_state, telemetry);

		Self { state }
	}

	fn bootstrap_core(content: String, path: Option<PathBuf>) -> (CoreStateBundle, WorkScheduler, LanguageLoader) {
		let language_loader = LanguageLoader::from_embedded();

		let view_manager = ViewManager::new(content, path.clone(), &language_loader);
		let buffer_id = ViewId(1);
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

		let core = CoreStateBundle {
			editor: EditorCore::new(view_manager, Workspace::default(), UndoManager::new()),
			windows: window_manager,
			focus,
			focus_epoch: focus::FocusEpoch::initial(),
			layout: LayoutManager::new(),
			viewport: Viewport::default(),
			frame: FrameState::default(),
		};

		(core, work_scheduler, language_loader)
	}

	fn bootstrap_runtime() -> RuntimeStateBundle {
		RuntimeStateBundle {
			runtime_work_queue: RuntimeWorkQueue::default(),
			runtime_kernel: RuntimeKernel::default(),
			runtime_active_cause_id: None,
			effects: crate::effects::sink::EffectSink::default(),
			flush_depth: 0,
			recorder: crate::runtime::recorder::EventRecorder::from_env(),
		}
	}

	fn bootstrap_integrations(work_scheduler: WorkScheduler) -> IntegrationStateBundle {
		IntegrationStateBundle {
			nu: crate::nu::coordinator::NuCoordinatorState::new(),
			lsp: LspSystem::new(),
			syntax_manager: xeno_syntax::SyntaxManager::new(xeno_syntax::SyntaxManagerCfg {
				max_concurrency: 2,
				..Default::default()
			}),
			work_scheduler,
			filesystem: crate::filesystem::FsService::new_with_runtime(),
		}
	}

	fn bootstrap_ui() -> UiStateBundle {
		UiStateBundle {
			ui: UiManager::new(),
			overlay_system: OverlaySystem::default(),
			notifications: crate::notifications::NotificationCenter::new(),
			render_cache: crate::render::cache::RenderCache::new(),
			#[cfg(feature = "lsp")]
			inlay_hint_cache: crate::lsp::inlay_hints::InlayHintCache::new(),
			#[cfg(feature = "lsp")]
			pull_diag_state: crate::lsp::pull_diagnostics::PullDiagState::new(),
			#[cfg(feature = "lsp")]
			semantic_token_cache: crate::lsp::semantic_tokens::SemanticTokenCache::new(),
		}
	}

	fn bootstrap_config(language_loader: LanguageLoader) -> ConfigStateBundle {
		ConfigStateBundle {
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
			lsp_catalog_ready: false,
		}
	}

	fn bootstrap_async(msg_tx: MsgSender, msg_rx: MsgReceiver) -> AsyncStateBundle {
		AsyncStateBundle {
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
		}
	}

	fn bootstrap_telemetry() -> TelemetryStateBundle {
		TelemetryStateBundle {
			metrics: std::sync::Arc::new(crate::metrics::EditorMetrics::new()),
			command_usage: crate::completion::CommandPaletteUsage::default(),
		}
	}

	fn assemble_editor_state(
		core: CoreStateBundle,
		runtime: RuntimeStateBundle,
		integration: IntegrationStateBundle,
		ui: UiStateBundle,
		config: ConfigStateBundle,
		async_state: AsyncStateBundle,
		telemetry: TelemetryStateBundle,
	) -> EditorState {
		EditorState {
			core,
			runtime,
			integration,
			ui,
			config,
			async_state,
			telemetry,
		}
	}

	/// Configure a language server.
	pub fn configure_language_server(&mut self, _language: impl Into<String>, _config: crate::lsp::api::LanguageServerConfig) {
		#[cfg(feature = "lsp")]
		self.state.integration.lsp.configure_server(_language, _config);
	}

	/// Removes a language server configuration.
	pub fn remove_language_server(&mut self, _language: &str) {
		#[cfg(feature = "lsp")]
		self.state.integration.lsp.remove_server(_language);
	}

	/// Returns total error count across all buffers.
	pub fn total_error_count(&self) -> usize {
		#[cfg(feature = "lsp")]
		{
			self.state.integration.lsp.total_error_count()
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
			self.state.integration.lsp.total_warning_count()
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
			self.state.integration.lsp.error_count(_buffer)
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
			self.state.integration.lsp.warning_count(_buffer)
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
			self.state.integration.lsp.get_diagnostics(_buffer)
		}
		#[cfg(not(feature = "lsp"))]
		{
			Vec::new()
		}
	}

	/// Shuts down all language servers.
	pub async fn shutdown_lsp(&self) {
		#[cfg(feature = "lsp")]
		self.state.integration.lsp.shutdown_all().await;
	}

	/// Shuts down filesystem indexing/search actors with a bounded graceful timeout.
	pub async fn shutdown_filesystem(&self) {
		let timeout = std::time::Duration::from_millis(250);
		let report = self
			.state
			.integration
			.filesystem
			.shutdown(xeno_worker::ActorShutdownMode::Graceful { timeout })
			.await;
		if report.service.timed_out() || report.indexer.timed_out() || report.search.timed_out() {
			tracing::warn!(
				service_timed_out = report.service.timed_out(),
				indexer_timed_out = report.indexer.timed_out(),
				search_timed_out = report.search.timed_out(),
				"filesystem graceful shutdown timed out; forcing immediate"
			);
			let _ = self.state.integration.filesystem.shutdown(xeno_worker::ActorShutdownMode::Immediate).await;
		}
	}

	/// Returns the base window.
	pub fn base_window(&self) -> &BaseWindow {
		self.state.core.windows.base_window()
	}

	/// Returns the base window mutably.
	pub fn base_window_mut(&mut self) -> &mut BaseWindow {
		self.state.core.windows.base_window_mut()
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
		&self.state.core.windows
	}

	#[inline]
	pub fn windows_mut(&mut self) -> &mut WindowManager {
		&mut self.state.core.windows
	}

	#[inline]
	pub fn focus(&self) -> &FocusTarget {
		&self.state.core.focus
	}

	#[inline]
	pub fn focus_mut(&mut self) -> &mut FocusTarget {
		&mut self.state.core.focus
	}

	#[inline]
	pub fn layout(&self) -> &LayoutManager {
		&self.state.core.layout
	}

	#[inline]
	pub fn layout_mut(&mut self) -> &mut LayoutManager {
		&mut self.state.core.layout
	}

	#[inline]
	pub fn viewport(&self) -> &Viewport {
		&self.state.core.viewport
	}

	#[inline]
	pub fn viewport_mut(&mut self) -> &mut Viewport {
		&mut self.state.core.viewport
	}

	#[inline]
	pub fn ui(&self) -> &UiManager {
		&self.state.ui.ui
	}

	#[inline]
	pub fn ui_mut(&mut self) -> &mut UiManager {
		&mut self.state.ui.ui
	}

	#[inline]
	pub fn frame(&self) -> &FrameState {
		&self.state.core.frame
	}

	#[inline]
	pub fn frame_mut(&mut self) -> &mut FrameState {
		&mut self.state.core.frame
	}

	#[inline]
	pub fn config(&self) -> &Config {
		&self.state.config.config
	}

	#[inline]
	pub fn config_mut(&mut self) -> &mut Config {
		&mut self.state.config.config
	}

	/// Sets keybinding overrides and invalidates the effective keymap cache.
	pub fn set_key_overrides(&mut self, keys: Option<xeno_registry::config::UnresolvedKeys>) {
		self.state.config.key_overrides = keys;
		self.state.config.keymap_cache.lock().take();
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
		self.state.config.keymap_behavior = preset.behavior;
		self.state.config.keymap_initial_mode = preset.initial_mode.clone();
		self.state.config.keymap_preset = preset;
		self.state.config.keymap_preset_spec = spec;
		self.state.config.keymap_cache.lock().take();
		let initial_mode = self.state.config.keymap_initial_mode.clone();
		self.buffer_mut().input.set_mode(initial_mode.clone());
		self.state.core.editor.buffers.set_initial_mode(initial_mode);
	}

	/// Returns the behavioral flags from the active keymap preset.
	pub fn keymap_behavior(&self) -> xeno_registry::keymaps::KeymapBehavior {
		self.state.config.keymap_behavior
	}

	/// Returns the initial mode from the active keymap preset.
	pub fn keymap_initial_mode(&self) -> xeno_primitives::Mode {
		self.state.config.keymap_initial_mode.clone()
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
		self.state.integration.nu.set_runtime(runtime);
	}

	/// Returns the currently loaded Nu runtime, if any.
	pub fn nu_runtime(&self) -> Option<&crate::nu::NuRuntime> {
		self.state.integration.nu.runtime()
	}

	/// Returns the Nu executor, creating one from the current runtime if the
	/// executor is missing (e.g. after a worker thread panic or first access).
	pub fn ensure_nu_executor(&mut self) -> Option<&crate::nu::executor::NuExecutor> {
		self.state.integration.nu.ensure_executor()
	}

	/// Returns the effective keymap for the current catalog version, preset, and overrides.
	pub fn effective_keymap(&self) -> Arc<KeymapSnapshot> {
		let catalog_version = xeno_registry::CATALOG.version_hash();
		let snap = xeno_registry::ACTIONS.snapshot();
		let overrides_hash = hash_unresolved_keys(self.state.config.key_overrides.as_ref());
		let preset_ptr = Arc::as_ptr(&self.state.config.keymap_preset) as usize;

		{
			let cache = self.state.config.keymap_cache.lock();
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
			Some(&self.state.config.keymap_preset),
			self.state.config.key_overrides.as_ref(),
		));
		let mut cache = self.state.config.keymap_cache.lock();
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
		self.state.ui.notifications.take_pending_render_items()
	}

	#[inline]
	pub fn notifications_clear_epoch(&self) -> u64 {
		self.state.ui.notifications.clear_epoch()
	}

	#[inline]
	#[cfg_attr(not(feature = "lsp"), allow(dead_code))]
	pub(crate) fn lsp(&self) -> &LspSystem {
		&self.state.integration.lsp
	}

	#[inline]
	#[cfg(feature = "lsp")]
	pub(crate) fn lsp_handle(&self) -> LspHandle {
		self.state.integration.lsp.handle()
	}

	#[inline]
	pub(crate) fn work_scheduler_mut(&mut self) -> &mut WorkScheduler {
		&mut self.state.integration.work_scheduler
	}

	/// Emits the editor-start lifecycle hook on the work scheduler.
	pub fn emit_editor_start_hook(&mut self) {
		emit_hook_sync_with(&HookContext::new(HookEventData::EditorStart), &mut self.state.integration.work_scheduler);
	}

	/// Emits the editor-quit lifecycle hook asynchronously.
	pub async fn emit_editor_quit_hook(&self) {
		emit_hook(&HookContext::new(HookEventData::EditorQuit)).await;
	}

	#[inline]
	pub(crate) fn overlays(&self) -> &OverlayStore {
		self.state.ui.overlay_system.store()
	}

	#[inline]
	pub(crate) fn overlays_mut(&mut self) -> &mut OverlayStore {
		self.state.ui.overlay_system.store_mut()
	}

	/// Returns the active modal controller kind for frontend policy.
	#[inline]
	pub fn overlay_kind(&self) -> Option<crate::overlay::OverlayControllerKind> {
		let active = self.state.ui.overlay_system.interaction().active()?;
		Some(active.controller.kind())
	}

	/// Returns a data-only pane plan for the active modal overlay session.
	#[inline]
	pub fn overlay_pane_render_plan(&self) -> Vec<crate::overlay::OverlayPaneRenderTarget> {
		self.state
			.ui
			.overlay_system
			.interaction()
			.active()
			.map_or_else(Vec::new, |active| active.session.pane_render_plan())
	}

	/// Returns the active modal pane rect for a role when available.
	#[inline]
	pub fn overlay_pane_rect(&self, role: crate::overlay::WindowRole) -> Option<crate::geometry::Rect> {
		let active = self.state.ui.overlay_system.interaction().active()?;
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
		self.state.core.frame.needs_redraw = false;
	}

	/// Prepares a frontend frame using a backend-neutral viewport.
	///
	/// This centralizes per-frame editor updates that were previously performed in
	/// frontend compositor code (viewport sync, UI dock planning, and separator
	/// hover animation activation).
	pub fn begin_frontend_frame(&mut self, viewport: Rect) -> FrontendFramePlan {
		self.state.core.frame.needs_redraw = false;
		self.ensure_syntax_for_buffers();
		self.state.core.viewport.width = Some(viewport.width);
		self.state.core.viewport.height = Some(viewport.height);

		let status_rows = self.statusline_rows().min(viewport.height);
		let main_rows = viewport.height.saturating_sub(status_rows);
		let main_area = Rect::new(viewport.x, viewport.y, viewport.width, main_rows);
		let status_area = Rect::new(viewport.x, viewport.y.saturating_add(main_rows), viewport.width, status_rows);

		let mut ui = std::mem::take(&mut self.state.ui.ui);
		ui.sync_utility_for_modal_overlay(self.utility_overlay_height_hint());
		ui.sync_utility_for_whichkey(self.whichkey_desired_height());
		let dock_layout = ui.compute_layout(main_area);
		let panel_render_plan = ui.panel_render_plan(&dock_layout);
		let doc_area = dock_layout.doc_area;
		self.state.core.viewport.doc_area = Some(doc_area);

		let activate_separator_hover = {
			let layout = &self.state.core.layout;
			layout.hovered_separator.is_none() && layout.separator_under_mouse.is_some() && !layout.is_mouse_fast()
		};
		if activate_separator_hover {
			let layout = &mut self.state.core.layout;
			let old_hover = layout.hovered_separator.take();
			layout.hovered_separator = layout.separator_under_mouse;
			if old_hover != layout.hovered_separator {
				layout.update_hover_animation(old_hover, layout.hovered_separator);
				self.state.core.frame.needs_redraw = true;
			}
		}
		if self.state.core.layout.animation_needs_redraw() {
			self.state.core.frame.needs_redraw = true;
		}
		if ui.take_wants_redraw() {
			self.state.core.frame.needs_redraw = true;
		}
		self.state.ui.ui = ui;

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
		&self.state.integration.syntax_manager
	}

	#[inline]
	pub fn syntax_manager_mut(&mut self) -> &mut xeno_syntax::SyntaxManager {
		&mut self.state.integration.syntax_manager
	}

	#[inline]
	pub fn render_cache(&self) -> &crate::render::cache::RenderCache {
		&self.state.ui.render_cache
	}

	#[inline]
	pub fn render_cache_mut(&mut self) -> &mut crate::render::cache::RenderCache {
		&mut self.state.ui.render_cache
	}

	#[inline]
	pub fn metrics(&self) -> &std::sync::Arc<crate::metrics::EditorMetrics> {
		&self.state.telemetry.metrics
	}

	#[inline]
	pub fn metrics_mut(&mut self) -> &mut std::sync::Arc<crate::metrics::EditorMetrics> {
		&mut self.state.telemetry.metrics
	}

	/// Returns a cloneable message sender for background tasks.
	#[inline]
	pub fn msg_tx(&self) -> MsgSender {
		self.state.async_state.msg_tx.clone()
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
		while let Ok(msg) = self.state.async_state.msg_rx.try_recv() {
			report.drained_count += 1;
			report.dirty |= msg.apply(self);
		}
		report
	}
}
