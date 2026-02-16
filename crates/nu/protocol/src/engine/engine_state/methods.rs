impl EngineState {
	pub fn new() -> Self {
		let (send, recv) = channel::<Mail>();

		Self {
			files: vec![],
			virtual_paths: vec![],
			vars: vec![
				Variable::new(Span::new(0, 0), Type::Any, false),
				Variable::new(Span::new(0, 0), Type::Any, false),
				Variable::new(Span::new(0, 0), Type::Any, false),
				Variable::new(Span::new(0, 0), Type::Any, false),
				Variable::new(Span::new(0, 0), Type::Any, false),
			],
			decls: Arc::new(vec![]),
			blocks: Arc::new(vec![]),
			modules: Arc::new(vec![Arc::new(Module::new(DEFAULT_OVERLAY_NAME.as_bytes().to_vec()))]),
			spans: vec![Span::unknown()],
			doccomments: Doccomments::new(),
			// make sure we have some default overlay:
			scope: ScopeFrame::with_empty_overlay(DEFAULT_OVERLAY_NAME.as_bytes().to_vec(), ModuleId::new(0), false),
			signal_handlers: None,
			signals: Signals::empty(),
			env_vars: Arc::new([(DEFAULT_OVERLAY_NAME.to_string(), HashMap::new())].into_iter().collect()),
			previous_env_vars: Arc::new(HashMap::new()),
			config: Arc::new(Config::default()),
			pipeline_externals_state: Arc::new((AtomicU32::new(0), AtomicU32::new(0))),
			repl_state: Arc::new(Mutex::new(ReplState {
				buffer: "".to_string(),
				cursor_pos: 0,
				accept: false,
			})),
			table_decl_id: None,
			config_path: HashMap::new(),
			history_enabled: true,
			history_session_id: 0,
			file: None,
			regex_cache: Arc::new(Mutex::new(LruCache::new(
				NonZeroUsize::new(REGEX_CACHE_SIZE).expect("tried to create cache of size zero"),
			))),
			is_interactive: false,
			is_login: false,
			is_lsp: false,
			is_mcp: false,
			startup_time: -1,
			is_debugging: IsDebugging::new(false),
			debugger: Arc::new(Mutex::new(Box::new(NoopDebugger))),
			report_log: Arc::default(),
			jobs: Arc::new(Mutex::new(Jobs::default())),
			current_job: CurrentJob {
				id: JobId::new(0),
				background_thread_job: None,
				mailbox: Arc::new(Mutex::new(Mailbox::new(recv))),
			},
			root_job_sender: send,
			exit_warning_given: Arc::new(AtomicBool::new(false)),
		}
	}

	pub fn signals(&self) -> &Signals {
		&self.signals
	}

	pub fn reset_signals(&mut self) {
		self.signals.reset();
		if let Some(ref handlers) = self.signal_handlers {
			handlers.run(SignalAction::Reset);
		}
	}

	pub fn set_signals(&mut self, signals: Signals) {
		self.signals = signals;
	}

	/// Merges a `StateDelta` onto the current state. These deltas come from a system, like the parser, that
	/// creates a new set of definitions and visible symbols in the current scope. We make this transactional
	/// as there are times when we want to run the parser and immediately throw away the results (namely:
	/// syntax highlighting and completions).
	///
	/// When we want to preserve what the parser has created, we can take its output (the `StateDelta`) and
	/// use this function to merge it into the global state.
	pub fn merge_delta(&mut self, mut delta: StateDelta) -> Result<(), ShellError> {
		// Take the mutable reference and extend the permanent state from the working set
		self.files.extend(delta.files);
		self.virtual_paths.extend(delta.virtual_paths);
		self.vars.extend(delta.vars);
		self.spans.extend(delta.spans);
		self.doccomments.merge_with(delta.doccomments);

		// Avoid potentially cloning the Arcs if we aren't adding anything
		if !delta.decls.is_empty() {
			Arc::make_mut(&mut self.decls).extend(delta.decls);
		}
		if !delta.blocks.is_empty() {
			Arc::make_mut(&mut self.blocks).extend(delta.blocks);
		}
		if !delta.modules.is_empty() {
			Arc::make_mut(&mut self.modules).extend(delta.modules);
		}

		let first = delta.scope.remove(0);

		for (delta_name, delta_overlay) in first.clone().overlays {
			if let Some((_, existing_overlay)) = self.scope.overlays.iter_mut().find(|(name, _)| name == &delta_name) {
				// Updating existing overlay
				for item in delta_overlay.decls.into_iter() {
					existing_overlay.decls.insert(item.0, item.1);
				}
				for item in delta_overlay.vars.into_iter() {
					existing_overlay.insert_variable(item.0, item.1);
				}
				for item in delta_overlay.modules.into_iter() {
					existing_overlay.modules.insert(item.0, item.1);
				}

				existing_overlay.visibility.merge_with(delta_overlay.visibility);
			} else {
				// New overlay was added to the delta
				self.scope.overlays.push((delta_name, delta_overlay));
			}
		}

		let mut activated_ids = self.translate_overlay_ids(&first);

		let mut removed_ids = vec![];

		for name in &first.removed_overlays {
			if let Some(overlay_id) = self.find_overlay(name) {
				removed_ids.push(overlay_id);
			}
		}

		// Remove overlays removed in delta
		self.scope.active_overlays.retain(|id| !removed_ids.contains(id));

		// Move overlays activated in the delta to be first
		self.scope.active_overlays.retain(|id| !activated_ids.contains(id));
		self.scope.active_overlays.append(&mut activated_ids);

		Ok(())
	}

	/// Merge the environment from the runtime Stack into the engine state
	pub fn merge_env(&mut self, stack: &mut Stack) -> Result<(), ShellError> {
		for mut scope in stack.env_vars.drain(..) {
			for (overlay_name, mut env) in Arc::make_mut(&mut scope).drain() {
				if let Some(env_vars) = Arc::make_mut(&mut self.env_vars).get_mut(&overlay_name) {
					// Updating existing overlay
					env_vars.extend(env.drain());
				} else {
					// Pushing a new overlay
					Arc::make_mut(&mut self.env_vars).insert(overlay_name, env);
				}
			}
		}

		let cwd = self.cwd(Some(stack))?;
		std::env::set_current_dir(cwd).map_err(|err| IoError::new_internal(err, "Could not set current dir", crate::location!()))?;

		if let Some(config) = stack.config.take() {
			// If config was updated in the stack, replace it.
			self.config = config;
		}

		Ok(())
	}

	/// Clean up unused variables from a Stack to prevent memory leaks.
	/// This removes variables that are no longer referenced by any overlay.
	pub fn cleanup_stack_variables(&mut self, stack: &mut Stack) {
		use std::collections::HashSet;

		let mut shadowed_vars = HashSet::new();
		for (_, frame) in self.scope.overlays.iter_mut() {
			shadowed_vars.extend(frame.shadowed_vars.to_owned());
			frame.shadowed_vars.clear();
		}

		// Remove variables from stack that are no longer referenced
		stack.vars.retain(|(var_id, _)| !shadowed_vars.contains(var_id));
	}

	pub fn active_overlay_ids<'a, 'b>(&'b self, removed_overlays: &'a [Vec<u8>]) -> impl DoubleEndedIterator<Item = &'b OverlayId> + 'a
	where
		'b: 'a,
	{
		self.scope
			.active_overlays
			.iter()
			.filter(|id| !removed_overlays.iter().any(|name| name == self.get_overlay_name(**id)))
	}

	pub fn active_overlays<'a, 'b>(&'b self, removed_overlays: &'a [Vec<u8>]) -> impl DoubleEndedIterator<Item = &'b OverlayFrame> + 'a
	where
		'b: 'a,
	{
		self.active_overlay_ids(removed_overlays).map(|id| self.get_overlay(*id))
	}

	pub fn active_overlay_names<'a, 'b>(&'b self, removed_overlays: &'a [Vec<u8>]) -> impl DoubleEndedIterator<Item = &'b [u8]> + 'a
	where
		'b: 'a,
	{
		self.active_overlay_ids(removed_overlays).map(|id| self.get_overlay_name(*id))
	}

	/// Translate overlay IDs from other to IDs in self
	fn translate_overlay_ids(&self, other: &ScopeFrame) -> Vec<OverlayId> {
		let other_names = other
			.active_overlays
			.iter()
			.map(|other_id| &other.overlays.get(other_id.get()).expect("internal error: missing overlay").0);

		other_names
			.map(|other_name| self.find_overlay(other_name).expect("internal error: missing overlay"))
			.collect()
	}

	pub fn last_overlay_name(&self, removed_overlays: &[Vec<u8>]) -> &[u8] {
		self.active_overlay_names(removed_overlays).last().expect("internal error: no active overlays")
	}

	pub fn last_overlay(&self, removed_overlays: &[Vec<u8>]) -> &OverlayFrame {
		self.active_overlay_ids(removed_overlays)
			.last()
			.map(|id| self.get_overlay(*id))
			.expect("internal error: no active overlays")
	}

	pub fn get_overlay_name(&self, overlay_id: OverlayId) -> &[u8] {
		&self.scope.overlays.get(overlay_id.get()).expect("internal error: missing overlay").0
	}

	pub fn get_overlay(&self, overlay_id: OverlayId) -> &OverlayFrame {
		&self.scope.overlays.get(overlay_id.get()).expect("internal error: missing overlay").1
	}

	pub fn render_env_vars(&self) -> HashMap<&str, &Value> {
		let mut result: HashMap<&str, &Value> = HashMap::new();

		for overlay_name in self.active_overlay_names(&[]) {
			let name = String::from_utf8_lossy(overlay_name);
			if let Some(env_vars) = self.env_vars.get(name.as_ref()) {
				result.extend(env_vars.iter().map(|(k, v)| (k.as_str(), v)));
			}
		}

		result
	}

	pub fn add_env_var(&mut self, name: String, val: Value) {
		let overlay_name = String::from_utf8_lossy(self.last_overlay_name(&[])).to_string();

		if let Some(env_vars) = Arc::make_mut(&mut self.env_vars).get_mut(&overlay_name) {
			env_vars.insert(name, val);
		} else {
			Arc::make_mut(&mut self.env_vars).insert(overlay_name, [(name, val)].into_iter().collect());
		}
	}

	pub fn get_env_var(&self, name: &str) -> Option<&Value> {
		for overlay_id in self.scope.active_overlays.iter().rev() {
			let overlay_name = String::from_utf8_lossy(self.get_overlay_name(*overlay_id));
			if let Some(env_vars) = self.env_vars.get(overlay_name.as_ref())
				&& let Some(val) = env_vars.get(name)
			{
				return Some(val);
			}
		}

		None
	}

	// Returns Some((name, value)) if found, None otherwise.
	// When updating environment variables, make sure to use
	// the same case (the returned "name") as the original
	// environment variable name.
	pub fn get_env_var_insensitive(&self, name: &str) -> Option<(&String, &Value)> {
		for overlay_id in self.scope.active_overlays.iter().rev() {
			let overlay_name = String::from_utf8_lossy(self.get_overlay_name(*overlay_id));
			if let Some(env_vars) = self.env_vars.get(overlay_name.as_ref())
				&& let Some(v) = env_vars.iter().find(|(k, _)| k.eq_ignore_case(name))
			{
				return Some((v.0, v.1));
			}
		}

		None
	}

	pub fn num_files(&self) -> usize {
		self.files.len()
	}

	pub fn num_virtual_paths(&self) -> usize {
		self.virtual_paths.len()
	}

	pub fn num_vars(&self) -> usize {
		self.vars.len()
	}

	pub fn num_decls(&self) -> usize {
		self.decls.len()
	}

	pub fn num_blocks(&self) -> usize {
		self.blocks.len()
	}

	pub fn num_modules(&self) -> usize {
		self.modules.len()
	}

	pub fn num_spans(&self) -> usize {
		self.spans.len()
	}
	pub fn print_vars(&self) {
		for var in self.vars.iter().enumerate() {
			println!("var{}: {:?}", var.0, var.1);
		}
	}

	pub fn print_decls(&self) {
		for decl in self.decls.iter().enumerate() {
			println!("decl{}: {:?}", decl.0, decl.1.signature());
		}
	}

	pub fn print_blocks(&self) {
		for block in self.blocks.iter().enumerate() {
			println!("block{}: {:?}", block.0, block.1);
		}
	}

	pub fn print_contents(&self) {
		for cached_file in self.files.iter() {
			let string = String::from_utf8_lossy(&cached_file.content);
			println!("{string}");
		}
	}

	/// Find the [`DeclId`](crate::DeclId) corresponding to a declaration with `name`.
	///
	/// Searches within active overlays, and filtering out overlays in `removed_overlays`.
	pub fn find_decl(&self, name: &[u8], removed_overlays: &[Vec<u8>]) -> Option<DeclId> {
		let mut visibility: Visibility = Visibility::new();

		for overlay_frame in self.active_overlays(removed_overlays).rev() {
			visibility.append(&overlay_frame.visibility);

			if let Some(decl_id) = overlay_frame.get_decl(name)
				&& visibility.is_decl_id_visible(&decl_id)
			{
				return Some(decl_id);
			}
		}

		None
	}

	/// Find the name of the declaration corresponding to `decl_id`.
	///
	/// Searches within active overlays, and filtering out overlays in `removed_overlays`.
	pub fn find_decl_name(&self, decl_id: DeclId, removed_overlays: &[Vec<u8>]) -> Option<&[u8]> {
		let mut visibility: Visibility = Visibility::new();

		for overlay_frame in self.active_overlays(removed_overlays).rev() {
			visibility.append(&overlay_frame.visibility);

			if visibility.is_decl_id_visible(&decl_id) {
				for (name, id) in overlay_frame.decls.iter() {
					if id == &decl_id {
						return Some(name);
					}
				}
			}
		}

		None
	}

	/// Find the [`OverlayId`](crate::OverlayId) corresponding to `name`.
	///
	/// Searches all overlays, not just active overlays. To search only in active overlays, use [`find_active_overlay`](EngineState::find_active_overlay)
	pub fn find_overlay(&self, name: &[u8]) -> Option<OverlayId> {
		self.scope.find_overlay(name)
	}

	/// Find the [`OverlayId`](crate::OverlayId) of the active overlay corresponding to `name`.
	///
	/// Searches only active overlays. To search in all overlays, use [`find_overlay`](EngineState::find_active_overlay)
	pub fn find_active_overlay(&self, name: &[u8]) -> Option<OverlayId> {
		self.scope.find_active_overlay(name)
	}

	/// Find the [`ModuleId`](crate::ModuleId) corresponding to `name`.
	///
	/// Searches within active overlays, and filtering out overlays in `removed_overlays`.
	pub fn find_module(&self, name: &[u8], removed_overlays: &[Vec<u8>]) -> Option<ModuleId> {
		for overlay_frame in self.active_overlays(removed_overlays).rev() {
			if let Some(module_id) = overlay_frame.modules.get(name) {
				return Some(*module_id);
			}
		}

		None
	}

	pub fn get_module_comments(&self, module_id: ModuleId) -> Option<&[Span]> {
		self.doccomments.get_module_comments(module_id)
	}

	pub fn which_module_has_decl(&self, decl_name: &[u8], removed_overlays: &[Vec<u8>]) -> Option<&[u8]> {
		for overlay_frame in self.active_overlays(removed_overlays).rev() {
			for (module_name, module_id) in overlay_frame.modules.iter() {
				let module = self.get_module(*module_id);
				if module.has_decl(decl_name) {
					return Some(module_name);
				}
			}
		}

		None
	}

	/// Apply a function to all commands. The function accepts a command name and its DeclId
	pub fn traverse_commands(&self, mut f: impl FnMut(&[u8], DeclId)) {
		for overlay_frame in self.active_overlays(&[]).rev() {
			for (name, decl_id) in &overlay_frame.decls {
				if overlay_frame.visibility.is_decl_id_visible(decl_id) {
					f(name, *decl_id);
				}
			}
		}
	}

	pub fn get_span_contents(&self, span: Span) -> &[u8] {
		for file in &self.files {
			if file.covered_span.contains_span(span) {
				return &file.content[(span.start - file.covered_span.start)..(span.end - file.covered_span.start)];
			}
		}
		&[0u8; 0]
	}

	/// If the span's content starts with the given prefix, return two subspans
	/// corresponding to this prefix, and the rest of the content.
	pub fn span_match_prefix(&self, span: Span, prefix: &[u8]) -> Option<(Span, Span)> {
		let contents = self.get_span_contents(span);

		if contents.starts_with(prefix) { span.split_at(prefix.len()) } else { None }
	}

	/// If the span's content ends with the given postfix, return two subspans
	/// corresponding to the rest of the content, and this postfix.
	pub fn span_match_postfix(&self, span: Span, prefix: &[u8]) -> Option<(Span, Span)> {
		let contents = self.get_span_contents(span);

		if contents.ends_with(prefix) {
			span.split_at(span.len() - prefix.len())
		} else {
			None
		}
	}

	/// Get the global config from the engine state.
	///
	/// Use [`Stack::get_config()`] instead whenever the `Stack` is available, as it takes into
	/// account local changes to `$env.config`.
	pub fn get_config(&self) -> &Arc<Config> {
		&self.config
	}

	pub fn set_config(&mut self, conf: impl Into<Arc<Config>>) {
		self.config = conf.into();
	}

	/// Fetch the configuration for a plugin
	///
	/// The `plugin` must match the registered name of a plugin.  For `plugin add
	/// nu_plugin_example` the plugin name to use will be `"example"`
	pub fn get_plugin_config(&self, plugin: &str) -> Option<&Value> {
		self.config.plugins.get(plugin)
	}

	/// Returns the configuration settings for command history or `None` if history is disabled
	pub fn history_config(&self) -> Option<HistoryConfig> {
		self.history_enabled.then(|| self.config.history)
	}

	pub fn get_var(&self, var_id: VarId) -> &Variable {
		self.vars.get(var_id.get()).expect("internal error: missing variable")
	}

	pub fn get_constant(&self, var_id: VarId) -> Option<&Value> {
		let var = self.get_var(var_id);
		var.const_val.as_ref()
	}

	pub fn generate_nu_constant(&mut self) {
		self.vars[NU_VARIABLE_ID.get()].const_val = Some(create_nu_constant(self, Span::unknown()));
	}

	pub fn get_decl(&self, decl_id: DeclId) -> &dyn Command {
		self.decls.get(decl_id.get()).expect("internal error: missing declaration").as_ref()
	}

	/// Get all commands within scope, sorted by the commands' names
	pub fn get_decls_sorted(&self, include_hidden: bool) -> Vec<(Vec<u8>, DeclId)> {
		let mut decls_map = HashMap::new();

		for overlay_frame in self.active_overlays(&[]) {
			let new_decls = if include_hidden {
				overlay_frame.decls.clone()
			} else {
				overlay_frame
					.decls
					.clone()
					.into_iter()
					.filter(|(_, id)| overlay_frame.visibility.is_decl_id_visible(id))
					.collect()
			};

			decls_map.extend(new_decls);
		}

		let mut decls: Vec<(Vec<u8>, DeclId)> = decls_map.into_iter().collect();

		decls.sort_by(|a, b| a.0.cmp(&b.0));
		decls
	}

	pub fn get_signature(&self, decl: &dyn Command) -> Signature {
		if let Some(block_id) = decl.block_id() {
			*self.blocks[block_id.get()].signature.clone()
		} else {
			decl.signature()
		}
	}

	/// Get signatures of all commands within scope with their decl ids.
	pub fn get_signatures_and_declids(&self, include_hidden: bool) -> Vec<(Signature, DeclId)> {
		self.get_decls_sorted(include_hidden)
			.into_iter()
			.map(|(_, id)| {
				let decl = self.get_decl(id);

				(self.get_signature(decl).update_from_command(decl), id)
			})
			.collect()
	}

	pub fn get_block(&self, block_id: BlockId) -> &Arc<Block> {
		self.blocks.get(block_id.get()).expect("internal error: missing block")
	}

	/// Optionally get a block by id, if it exists
	///
	/// Prefer to use [`.get_block()`](Self::get_block) in most cases - `BlockId`s that don't exist
	/// are normally a compiler error. This only exists to stop plugins from crashing the engine if
	/// they send us something invalid.
	pub fn try_get_block(&self, block_id: BlockId) -> Option<&Arc<Block>> {
		self.blocks.get(block_id.get())
	}

	pub fn get_module(&self, module_id: ModuleId) -> &Module {
		self.modules.get(module_id.get()).expect("internal error: missing module")
	}

	pub fn get_virtual_path(&self, virtual_path_id: VirtualPathId) -> &(String, VirtualPath) {
		self.virtual_paths.get(virtual_path_id.get()).expect("internal error: missing virtual path")
	}

	pub fn next_span_start(&self) -> usize {
		if let Some(cached_file) = self.files.last() {
			cached_file.covered_span.end
		} else {
			0
		}
	}

	pub fn files(&self) -> impl DoubleEndedIterator<Item = &CachedFile> + ExactSizeIterator<Item = &CachedFile> {
		self.files.iter()
	}

	pub fn add_file(&mut self, filename: Arc<str>, content: Arc<[u8]>) -> FileId {
		let next_span_start = self.next_span_start();
		let next_span_end = next_span_start + content.len();

		let covered_span = Span::new(next_span_start, next_span_end);

		self.files.push(CachedFile {
			name: filename,
			content,
			covered_span,
		});

		FileId::new(self.num_files() - 1)
	}

	pub fn set_config_path(&mut self, key: &str, val: PathBuf) {
		self.config_path.insert(key.to_string(), val);
	}

	pub fn get_config_path(&self, key: &str) -> Option<&PathBuf> {
		self.config_path.get(key)
	}

	pub fn build_desc(&self, spans: &[Span]) -> (String, String) {
		let comment_lines: Vec<&[u8]> = spans.iter().map(|span| self.get_span_contents(*span)).collect();
		build_desc(&comment_lines)
	}

	pub fn build_module_desc(&self, module_id: ModuleId) -> Option<(String, String)> {
		self.get_module_comments(module_id).map(|comment_spans| self.build_desc(comment_spans))
	}

	/// Returns the current working directory, which is guaranteed to be canonicalized.
	///
	/// Returns an empty String if $env.PWD doesn't exist.
	#[deprecated(since = "0.92.3", note = "please use `EngineState::cwd()` instead")]
	pub fn current_work_dir(&self) -> String {
		self.cwd(None).map(|path| path.to_string_lossy().to_string()).unwrap_or_default()
	}

	/// Returns the current working directory, which is guaranteed to be an
	/// absolute path without trailing slashes (unless it's the root path), but
	/// might contain symlink components.
	///
	/// If `stack` is supplied, also considers modifications to the working
	/// directory on the stack that have yet to be merged into the engine state.
	pub fn cwd(&self, stack: Option<&Stack>) -> Result<AbsolutePathBuf, ShellError> {
		// Helper function to create a simple generic error.
		fn error(msg: &str, cwd: impl AsRef<xeno_nu_path::Path>) -> ShellError {
			ShellError::GenericError {
				error: msg.into(),
				msg: format!("$env.PWD = {}", cwd.as_ref().display()),
				span: None,
				help: Some("Use `cd` to reset $env.PWD into a good state".into()),
				inner: vec![],
			}
		}

		// Retrieve $env.PWD from the stack or the engine state.
		let pwd = if let Some(stack) = stack {
			stack.get_env_var(self, "PWD")
		} else {
			self.get_env_var("PWD")
		};

		let pwd = pwd.ok_or_else(|| error("$env.PWD not found", ""))?;

		if let Ok(pwd) = pwd.as_str() {
			let path = AbsolutePathBuf::try_from(pwd).map_err(|_| error("$env.PWD is not an absolute path", pwd))?;

			// Technically, a root path counts as "having trailing slashes", but
			// for the purpose of PWD, a root path is acceptable.
			if path.parent().is_some() && xeno_nu_path::has_trailing_slash(path.as_ref()) {
				Err(error("$env.PWD contains trailing slashes", &path))
			} else if !path.exists() {
				Err(error("$env.PWD points to a non-existent directory", &path))
			} else if !path.is_dir() {
				Err(error("$env.PWD points to a non-directory", &path))
			} else {
				Ok(path)
			}
		} else {
			Err(error("$env.PWD is not a string", format!("{pwd:?}")))
		}
	}

	/// Like `EngineState::cwd()`, but returns a String instead of a PathBuf for convenience.
	pub fn cwd_as_string(&self, stack: Option<&Stack>) -> Result<String, ShellError> {
		let cwd = self.cwd(stack)?;
		cwd.into_os_string().into_string().map_err(|err| ShellError::NonUtf8Custom {
			msg: format!("The current working directory is not a valid utf-8 string: {err:?}"),
			span: Span::unknown(),
		})
	}

	// TODO: see if we can completely get rid of this
	pub fn get_file_contents(&self) -> &[CachedFile] {
		&self.files
	}

	pub fn get_startup_time(&self) -> i64 {
		self.startup_time
	}

	pub fn set_startup_time(&mut self, startup_time: i64) {
		self.startup_time = startup_time;
	}

	pub fn activate_debugger(&self, debugger: Box<dyn Debugger>) -> Result<(), PoisonDebuggerError<'_>> {
		let mut locked_debugger = self.debugger.lock()?;
		*locked_debugger = debugger;
		locked_debugger.activate();
		self.is_debugging.0.store(true, Ordering::Relaxed);
		Ok(())
	}

	pub fn deactivate_debugger(&self) -> Result<Box<dyn Debugger>, PoisonDebuggerError<'_>> {
		let mut locked_debugger = self.debugger.lock()?;
		locked_debugger.deactivate();
		let ret = std::mem::replace(&mut *locked_debugger, Box::new(NoopDebugger));
		self.is_debugging.0.store(false, Ordering::Relaxed);
		Ok(ret)
	}

	pub fn is_debugging(&self) -> bool {
		self.is_debugging.0.load(Ordering::Relaxed)
	}

	pub fn recover_from_panic(&mut self) {
		if Mutex::is_poisoned(&self.repl_state) {
			self.repl_state = Arc::new(Mutex::new(ReplState {
				buffer: "".to_string(),
				cursor_pos: 0,
				accept: false,
			}));
		}
		if Mutex::is_poisoned(&self.jobs) {
			self.jobs = Arc::new(Mutex::new(Jobs::default()));
		}
		if Mutex::is_poisoned(&self.regex_cache) {
			self.regex_cache = Arc::new(Mutex::new(LruCache::new(
				NonZeroUsize::new(REGEX_CACHE_SIZE).expect("tried to create cache of size zero"),
			)));
		}
	}

	/// Add new span and return its ID
	pub fn add_span(&mut self, span: Span) -> SpanId {
		self.spans.push(span);
		SpanId::new(self.num_spans() - 1)
	}

	/// Find ID of a span (should be avoided if possible)
	pub fn find_span_id(&self, span: Span) -> Option<SpanId> {
		self.spans.iter().position(|sp| sp == &span).map(SpanId::new)
	}

	// Determines whether the current state is being held by a background job
	pub fn is_background_job(&self) -> bool {
		self.current_job.background_thread_job.is_some()
	}

	// Gets the thread job entry
	pub fn current_thread_job(&self) -> Option<&ThreadJob> {
		self.current_job.background_thread_job.as_ref()
	}
}

impl GetSpan for &EngineState {
	/// Get existing span
	fn get_span(&self, span_id: SpanId) -> Span {
		*self.spans.get(span_id.get()).expect("internal error: missing span")
	}
}
