use tome_cabi_types::{TomeCommandContextV1, TomeStatus, TomeStr};

use crate::editor::Editor;
use crate::plugin::manager::{HOST_V2, PluginManager};

impl Editor {
	pub fn submit_plugin_panel(&mut self, id: u64) {
		use tome_cabi_types::TomeChatRole;

		use crate::plugin::panels::ChatItem;

		if let Some(panel) = self.plugins.panels.get_mut(&id) {
			let text = panel.input.to_string();
			if text.trim().is_empty() {
				return;
			}

			panel.transcript.push(ChatItem {
				_role: TomeChatRole::User,
				_text: text.clone(),
			});
			panel.input = "".into();
			panel.input_cursor = 0;

			let text_tome = TomeStr {
				ptr: text.as_ptr(),
				len: text.len(),
			};

			if let Some(owner_id) = self.plugins.panel_owners.get(&id).cloned()
				&& let Some(plugin) = self.plugins.plugins.get(&owner_id)
				&& let Some(on_submit) = plugin.guest.on_panel_submit
			{
				use crate::plugin::manager::PluginContextGuard;
				let ed_ptr = self as *mut Editor;
				let mgr_ptr = unsafe { &mut (*ed_ptr).plugins as *mut _ };
				let _guard = unsafe { PluginContextGuard::new(mgr_ptr, ed_ptr, &owner_id) };
				on_submit(id, text_tome);
			}
		}
	}

	pub fn try_execute_plugin_command(&mut self, full_name: &str, args: &[&str]) -> bool {
		let cmd = match self.plugins.commands.get(full_name) {
			Some(c) => c,
			None => return false,
		};

		let plugin_id = cmd.plugin_id.clone();
		let handler = cmd.handler;

		let arg_tome_strs: Vec<TomeStr> = args
			.iter()
			.map(|s| TomeStr {
				ptr: s.as_ptr(),
				len: s.len(),
			})
			.collect();

		let mut ctx = TomeCommandContextV1 {
			argc: args.len(),
			argv: arg_tome_strs.as_ptr(),
			host: &HOST_V2,
		};

		let status = {
			use crate::plugin::manager::PluginContextGuard;
			let ed_ptr = self as *mut Editor;
			let mgr_ptr = unsafe { &mut (*ed_ptr).plugins as *mut _ };
			let _guard = unsafe { PluginContextGuard::new(mgr_ptr, ed_ptr, &plugin_id) };
			handler(&mut ctx)
		};

		if status != TomeStatus::Ok {
			self.show_error(format!(
				"Command {} failed with status {:?}",
				full_name, status
			));
		}
		true
	}

	pub fn autoload_plugins(&mut self) {
		let mgr_ptr = &mut self.plugins as *mut PluginManager;
		unsafe { (*mgr_ptr).autoload(self) };
	}

	pub fn save_plugin_config(&mut self) {
		self.plugins.save_config();
	}

	pub(crate) fn poll_ipc(&mut self) {
		let mut reloads = Vec::new();
		if let Some(ipc) = &self.ipc {
			while let Some(msg) = ipc.poll() {
				match msg {
					crate::ipc::IpcMessage::ReloadPlugin(id) => {
						reloads.push(id);
					}
				}
			}
		}

		for id in reloads {
			if let Err(e) = self.plugin_command(&["reload", &id]) {
				self.show_error(format!("IPC reload failed: {}", e));
			} else {
				self.show_message(format!("IPC: Reloaded plugin {}", id));
			}
		}
	}

	pub fn poll_plugins(&mut self) {
		self.poll_plugins_internal();
	}

	pub(crate) fn poll_plugins_internal(&mut self) {
		self.poll_ipc();
		use crate::plugin::manager::PluginContextGuard;
		let mut events = Vec::new();
		let plugin_ids: Vec<String> = self.plugins.plugins.keys().cloned().collect();
		for id in plugin_ids {
			if let Some(plugin) = self.plugins.plugins.get(&id)
				&& let Some(poll_event) = plugin.guest.poll_event
			{
				let ed_ptr = self as *mut Editor;
				let mgr_ptr = unsafe { &mut (*ed_ptr).plugins as *mut _ };
				let _guard = unsafe { PluginContextGuard::new(mgr_ptr, ed_ptr, &id) };
				loop {
					let mut event =
						std::mem::MaybeUninit::<tome_cabi_types::TomePluginEventV1>::uninit();
					let has_event = poll_event(event.as_mut_ptr());
					if has_event.0 == 0 {
						break;
					}
					let event = unsafe { event.assume_init() };
					events.push((id.clone(), event));
				}
			}
		}

		for (id, event) in events {
			self.handle_plugin_event(&id, event);
		}
	}

	pub(crate) fn handle_plugin_event(
		&mut self,
		plugin_id: &str,
		event: tome_cabi_types::TomePluginEventV1,
	) {
		use tome_cabi_types::TomePluginEventKind;

		use crate::plugin::manager::tome_owned_to_string;
		use crate::plugin::panels::ChatItem;

		let free_str_fn = self
			.plugins
			.plugins
			.get(plugin_id)
			.and_then(|p| p.guest.free_str);

		match event.kind {
			TomePluginEventKind::PanelAppend => {
				if self
					.plugins
					.panel_owners
					.get(&event.panel_id)
					.map(|s| s.as_str())
					== Some(plugin_id)
					&& let Some(panel) = self.plugins.panels.get_mut(&event.panel_id)
					&& let Some(text) = tome_owned_to_string(event.text)
				{
					panel.transcript.push(ChatItem {
						_role: event.role,
						_text: text,
					});
				}
			}
			TomePluginEventKind::PanelSetOpen => {
				if self
					.plugins
					.panel_owners
					.get(&event.panel_id)
					.map(|s| s.as_str())
					== Some(plugin_id)
					&& let Some(panel) = self.plugins.panels.get_mut(&event.panel_id)
				{
					panel.open = event.bool_val.0 != 0;
				}
			}
			TomePluginEventKind::ShowMessage => {
				if let Some(text) = tome_owned_to_string(event.text) {
					self.show_message(text);
				}
			}
			TomePluginEventKind::RequestPermission => {
				let req = unsafe { &*event.permission_request };
				let prompt = tome_owned_to_string(req.prompt).unwrap_or_default();
				let options_slice =
					unsafe { std::slice::from_raw_parts(req.options, req.options_len) };
				let mut options = Vec::new();
				for opt in options_slice {
					options.push((
						tome_owned_to_string(opt.option_id).unwrap_or_default(),
						tome_owned_to_string(opt.label).unwrap_or_default(),
					));
				}

				self.pending_permissions
					.push(crate::plugin::manager::PendingPermission {
						plugin_id: plugin_id.to_string(),
						request_id: event.permission_request_id,
						_prompt: prompt.clone(),
						_options: options.clone(),
					});

				self.show_message(format!(
					"Permission requested: {}. Use :permit {} <option>",
					prompt, event.permission_request_id,
				));
			}
		}

		if let Some(free_str) = free_str_fn
			&& !event.text.ptr.is_null()
		{
			free_str(event.text);
		}

		if !event.permission_request.is_null()
			&& let Some(free_perm) = self
				.plugins
				.plugins
				.get(plugin_id)
				.and_then(|p| p.guest.free_permission_request)
		{
			free_perm(event.permission_request);
		}
	}
}
