use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};
use serde::{Deserialize, Serialize};
use tome_cabi_types::{
	TOME_C_ABI_VERSION_V2, TomeBool, TomeChatRole, TomeCommandContextV1, TomeCommandSpecV1,
	TomeGuestV2, TomeHostPanelApiV1, TomeHostV2, TomeMessageKind, TomeOwnedStr, TomePanelId,
	TomePanelKind, TomePluginEntryV2, TomeStatus, TomeStr,
};

use super::panels::{ChatItem, ChatPanelState};
use crate::editor::Editor;

thread_local! {
	pub(crate) static ACTIVE_MANAGER: RefCell<Option<*mut PluginManager>> = const { RefCell::new(None) };
	pub(crate) static ACTIVE_EDITOR: RefCell<Option<*mut Editor>> = const { RefCell::new(None) };
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginManifest {
	pub id: String,
	pub name: String,
	pub version: String,
	pub abi: u32,
	pub library_path: Option<String>,
	pub dev_library_path: Option<String>,
	pub description: Option<String>,
	pub homepage: Option<String>,
	pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PluginStatus {
	Installed,
	Loaded,
	Failed(String),
	Disabled,
}

pub struct PluginEntry {
	pub manifest: PluginManifest,
	pub path: PathBuf, // directory containing plugin.toml
	pub status: PluginStatus,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TomeConfig {
	pub plugins: PluginsConfig,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PluginsConfig {
	#[serde(default)]
	pub autoload: bool,
	pub enabled: Vec<String>,
}

const HOST_PANEL_API_V1: TomeHostPanelApiV1 = TomeHostPanelApiV1 {
	create: host_panel_create,
	set_open: host_panel_set_open,
	set_focused: host_panel_set_focused,
	append_transcript: host_panel_append_transcript,
	request_redraw: host_request_redraw,
};

/// Stable host vtable storage for plugin lifetime.
///
/// Plugins may keep the host pointer around after init. This must not point
/// at stack locals (use-after-return).
pub(crate) static HOST_V2: TomeHostV2 = TomeHostV2 {
	abi_version: TOME_C_ABI_VERSION_V2,
	log: Some(host_log),
	panel: HOST_PANEL_API_V1,
	show_message: host_show_message,
	insert_text: host_insert_text,
	register_command: Some(host_register_command),
	get_current_path: Some(host_get_current_path),
	free_str: Some(host_free_str),
	fs_read_text: Some(host_fs_read_text),
	fs_write_text: Some(host_fs_write_text),
};

pub struct PendingPermission {
	pub plugin_id: String,
	pub request_id: u64,
	pub _prompt: String,
	pub _options: Vec<(String, String)>, // id, label
}

pub struct LoadedPlugin {
	#[allow(dead_code)]
	pub lib: Library,
	pub guest: TomeGuestV2,
	pub id: String,
}

impl Drop for LoadedPlugin {
	fn drop(&mut self) {
		if let Some(shutdown) = self.guest.shutdown {
			ACTIVE_MANAGER.with(|mgr_ctx| {
				ACTIVE_EDITOR.with(|ed_ctx| {
					if let (Some(mgr_ptr), Some(ed_ptr)) = (*mgr_ctx.borrow(), *ed_ctx.borrow()) {
						let _guard = unsafe { PluginContextGuard::new(mgr_ptr, ed_ptr, &self.id) };
						shutdown();
					} else {
						// Fallback if context is missing - but we should ensure it's set.
						shutdown();
					}
				})
			});
		}
	}
}

pub struct PluginCommand {
	pub plugin_id: String,
	#[allow(dead_code)]
	pub namespace: String,
	#[allow(dead_code)]
	pub name: String,
	pub handler: extern "C" fn(ctx: *mut TomeCommandContextV1) -> TomeStatus,
	#[allow(dead_code)]
	pub user_data: *mut core::ffi::c_void,
}

pub struct PluginManager {
	pub plugins: HashMap<String, LoadedPlugin>,
	pub entries: HashMap<String, PluginEntry>,
	pub config: TomeConfig,
	pub commands: HashMap<String, PluginCommand>,
	pub panels: HashMap<u64, ChatPanelState>,
	pub panel_owners: HashMap<u64, String>, // panel_id -> plugin_id
	next_panel_id: u64,
	current_namespace: Option<String>,
	pub(crate) current_plugin_id: Option<String>,

	pub plugins_open: bool,
	pub plugins_focused: bool,
	pub plugins_selected_idx: usize,
}

impl PluginManager {
	pub fn new() -> Self {
		Self {
			plugins: HashMap::new(),
			entries: HashMap::new(),
			config: TomeConfig::default(),
			commands: HashMap::new(),
			panels: HashMap::new(),
			panel_owners: HashMap::new(),
			next_panel_id: 1,
			current_namespace: None,
			current_plugin_id: None,
			plugins_open: false,
			plugins_focused: false,
			plugins_selected_idx: 0,
		}
	}

	pub fn load(&mut self, ed: &mut Editor, id: &str) -> Result<(), String> {
		let lib_path = {
			let entry = self
				.entries
				.get(id)
				.ok_or_else(|| format!("Plugin {} not found", id))?;

			if entry.manifest.abi != TOME_C_ABI_VERSION_V2 {
				return Err(format!(
					"Incompatible ABI version in manifest: host={}, plugin={}",
					TOME_C_ABI_VERSION_V2, entry.manifest.abi
				));
			}

			if let Some(dev_path) = &entry.manifest.dev_library_path {
				PathBuf::from(dev_path)
			} else if let Some(rel_path) = &entry.manifest.library_path {
				entry.path.join(rel_path)
			} else {
				return Err(format!("Plugin {} has no library_path", id));
			}
		};

		if !lib_path.exists() {
			return Err(format!("Library not found at {:?}", lib_path));
		}

		// Unload if already loaded. We MUST set context before removing from HashMap to ensure clean shutdown.
		if self.plugins.contains_key(id) {
			let mgr_ptr = self as *mut PluginManager;
			let ed_ptr = ed as *mut Editor;
			unsafe {
				let _guard = PluginContextGuard::new(mgr_ptr, ed_ptr, id);
				self.plugins.remove(id);
			}
			// Also remove commands from this plugin
			self.commands.retain(|_, cmd| cmd.plugin_id != id);
		}

		match self.load_at_path(ed, &lib_path, id) {
			Ok(()) => {
				if let Some(entry) = self.entries.get_mut(id) {
					entry.status = PluginStatus::Loaded;
				}
				Ok(())
			}
			Err(e) => {
				if let Some(entry) = self.entries.get_mut(id) {
					entry.status = PluginStatus::Failed(e.clone());
				}
				Err(e)
			}
		}
	}

	fn load_at_path(&mut self, ed: &mut Editor, path: &Path, id: &str) -> Result<(), String> {
		let lib =
			unsafe { Library::new(path) }.map_err(|e| format!("Failed to load library: {}", e))?;

		let entry: Symbol<TomePluginEntryV2> = unsafe {
			lib.get(b"tome_plugin_entry_v2\0")
				.map_err(|_| "Missing entry symbol 'tome_plugin_entry_v2'")?
		};

		let mut guest = unsafe { std::mem::zeroed::<TomeGuestV2>() };

		self.current_plugin_id = Some(id.to_string());

		let mgr_ptr = self as *mut PluginManager;
		let ed_ptr = ed as *mut Editor;

		let status = unsafe {
			let _guard = PluginContextGuard::new(mgr_ptr, ed_ptr, id);
			entry(&HOST_V2, &mut guest)
		};

		if status != TomeStatus::Ok {
			self.current_plugin_id = None;
			return Err(format!("Plugin entry failed with status {:?}", status));
		}

		if guest.abi_version != TOME_C_ABI_VERSION_V2 {
			return Err(format!(
				"Incompatible ABI version: host={}, guest={}",
				TOME_C_ABI_VERSION_V2, guest.abi_version
			));
		}

		let namespace = tome_str_to_str(&guest.namespace).to_string();
		self.current_namespace = Some(namespace);

		// Call init
		if let Some(init) = guest.init {
			let status = unsafe {
				let _guard = PluginContextGuard::new(mgr_ptr, ed_ptr, id);
				init(&HOST_V2)
			};
			if status != TomeStatus::Ok {
				self.current_namespace = None;
				self.current_plugin_id = None;
				return Err(format!("Plugin init failed with status {:?}", status));
			}
		}

		self.plugins.insert(
			id.to_string(),
			LoadedPlugin {
				lib,
				guest,
				id: id.to_string(),
			},
		);

		self.current_namespace = None;
		self.current_plugin_id = None;

		Ok(())
	}

	pub fn discover_plugins(&mut self) {
		let dirs = vec![
			std::env::var("TOME_PLUGIN_DIR").ok().map(PathBuf::from),
			home::home_dir().map(|h| h.join(".config/tome/plugins")),
		];

		for dir in dirs.into_iter().flatten() {
			if !dir.exists() {
				continue;
			}
			if let Ok(entries) = std::fs::read_dir(dir) {
				for entry in entries.flatten() {
					let path = entry.path();
					if path.is_dir() {
						let manifest_path = path.join("plugin.toml");
						if manifest_path.exists()
							&& let Ok(content) = std::fs::read_to_string(&manifest_path)
							&& let Ok(manifest) = toml::from_str::<PluginManifest>(&content)
						{
							self.entries.insert(
								manifest.id.clone(),
								PluginEntry {
									manifest,
									path: path.clone(),
									status: PluginStatus::Installed,
								},
							);
						}
					}
				}
			}
		}
	}

	pub fn load_config(&mut self) {
		let config_path = home::home_dir().map(|h| h.join(".config/tome/config.toml"));
		if let Some(path) = config_path
			&& path.exists()
			&& let Ok(content) = std::fs::read_to_string(path)
			&& let Ok(config) = toml::from_str::<TomeConfig>(&content)
		{
			self.config = config;
		}
	}

	pub fn save_config(&mut self) {
		let config_dir = home::home_dir().map(|h| h.join(".config/tome"));
		if let Some(dir) = config_dir {
			if !dir.exists() {
				if let Err(e) = std::fs::create_dir_all(&dir) {
					eprintln!("Failed to create config directory {:?}: {}", dir, e);
					return;
				}
			}
			let path = dir.join("config.toml");
			match toml::to_string(&self.config) {
				Ok(content) => {
					if let Err(e) = std::fs::write(&path, content) {
						eprintln!("Failed to write config file {:?}: {}", path, e);
					}
				}
				Err(e) => {
					eprintln!("Failed to serialize config: {}", e);
				}
			}
		}
	}

	pub fn autoload(&mut self, ed: &mut Editor) {
		self.discover_plugins();
		self.load_config();

		if !self.config.plugins.autoload {
			if !self.config.plugins.enabled.is_empty() {
				eprintln!(
					"Tome plugin autoloading is disabled (plugins.autoload = false in config.toml)."
				);
			}
			return;
		}

		// Update status for disabled plugins
		for (id, entry) in &mut self.entries {
			if !self.config.plugins.enabled.contains(id) {
				entry.status = PluginStatus::Disabled;
			}
		}

		let enabled = self.config.plugins.enabled.clone();
		for id in enabled {
			if let Err(e) = self.load(ed, &id) {
				eprintln!("Failed to load plugin {}: {}", id, e);
			}
		}
	}

	pub fn register_command(&mut self, spec: TomeCommandSpecV1) {
		let (namespace, plugin_id) = match (&self.current_namespace, &self.current_plugin_id) {
			(Some(ns), Some(id)) => (ns.clone(), id.clone()),
			_ => {
				eprintln!("Warning: register_command called outside of plugin init");
				return;
			}
		};

		if let Some(handler) = spec.handler {
			let name = tome_str_to_str(&spec.name).to_string();
			let full_name = format!("{}.{}", namespace, name);

			use std::collections::hash_map::Entry;
			match self.commands.entry(full_name) {
				Entry::Occupied(oe) => {
					eprintln!("Warning: Command {} already registered, skipping", oe.key());
				}
				Entry::Vacant(ve) => {
					ve.insert(PluginCommand {
						plugin_id: plugin_id.clone(),
						namespace: namespace.clone(),
						name,
						handler,
						user_data: spec.user_data,
					});
				}
			}

			// Also handle aliases
			if !spec.aliases.ptr.is_null() && spec.aliases.len > 0 {
				let aliases =
					unsafe { std::slice::from_raw_parts(spec.aliases.ptr, spec.aliases.len) };
				for alias in aliases {
					let alias_name = tome_str_to_str(alias).to_string();
					let full_alias = format!("{}.{}", namespace, alias_name);
					match self.commands.entry(full_alias) {
						Entry::Occupied(oe) => {
							eprintln!("Warning: Alias {} already registered, skipping", oe.key());
						}
						Entry::Vacant(ve) => {
							ve.insert(PluginCommand {
								plugin_id: plugin_id.clone(),
								namespace: namespace.clone(),
								name: alias_name,
								handler,
								user_data: spec.user_data,
							});
						}
					}
				}
			}
		}
	}
}

pub struct PluginContextGuard {
	old_mgr: Option<*mut PluginManager>,
	old_ed: Option<*mut Editor>,
	old_plugin_id: Option<String>,
	mgr_ptr: *mut PluginManager,
}

impl PluginContextGuard {
	pub unsafe fn new(mgr_ptr: *mut PluginManager, ed_ptr: *mut Editor, plugin_id: &str) -> Self {
		let old_mgr = ACTIVE_MANAGER.with(|ctx| ctx.replace(Some(mgr_ptr)));
		let old_ed = ACTIVE_EDITOR.with(|ctx| ctx.replace(Some(ed_ptr)));
		let (old_plugin_id, mgr_ptr_ref) =
			unsafe { ((*mgr_ptr).current_plugin_id.clone(), &mut *mgr_ptr) };
		mgr_ptr_ref.current_plugin_id = Some(plugin_id.to_string());
		Self {
			old_mgr,
			old_ed,
			old_plugin_id,
			mgr_ptr,
		}
	}
}

impl Drop for PluginContextGuard {
	fn drop(&mut self) {
		ACTIVE_MANAGER.with(|ctx| ctx.replace(self.old_mgr));
		ACTIVE_EDITOR.with(|ctx| ctx.replace(self.old_ed));
		unsafe {
			(*self.mgr_ptr).current_plugin_id = self.old_plugin_id.take();
		}
	}
}

pub fn tome_str_to_str(ts: &TomeStr) -> &str {
	if ts.ptr.is_null() {
		return "";
	}
	unsafe {
		let slice = std::slice::from_raw_parts(ts.ptr, ts.len);
		std::str::from_utf8(slice).unwrap_or("<invalid utf-8>")
	}
}

pub fn tome_owned_to_string(tos: TomeOwnedStr) -> Option<String> {
	if tos.ptr.is_null() {
		return None;
	}
	unsafe {
		let slice = std::slice::from_raw_parts(tos.ptr, tos.len);
		Some(String::from_utf8_lossy(slice).into_owned())
	}
}

// Host callbacks

pub(crate) extern "C" fn host_log(msg: TomeStr) {
	let s = tome_str_to_str(&msg);
	ACTIVE_EDITOR.with(|ctx| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			ed.show_message(format!("[plugin] {}", s));
		}
	});
}

pub(crate) extern "C" fn host_panel_create(kind: TomePanelKind, title: TomeStr) -> TomePanelId {
	ACTIVE_MANAGER.with(|ctx| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			let id = mgr.next_panel_id;
			mgr.next_panel_id += 1;

			if let Some(plugin_id) = &mgr.current_plugin_id {
				mgr.panel_owners.insert(id, plugin_id.clone());
			}

			let title_str = tome_str_to_str(&title).to_string();
			match kind {
				TomePanelKind::Chat => {
					mgr.panels.insert(id, ChatPanelState::new(id, title_str));
				}
			}
			id
		} else {
			0
		}
	})
}

pub(crate) extern "C" fn host_panel_set_open(id: TomePanelId, open: TomeBool) {
	ACTIVE_MANAGER.with(|ctx| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			if mgr.panel_owners.get(&id) == mgr.current_plugin_id.as_ref()
				&& let Some(panel) = mgr.panels.get_mut(&id)
			{
				panel.open = open.0 != 0;
				if panel.open {
					for (pid, p) in &mut mgr.panels {
						if *pid != id {
							p.open = false;
						}
					}
				}
			}
		}
	})
}

pub(crate) extern "C" fn host_panel_set_focused(id: TomePanelId, focused: TomeBool) {
	ACTIVE_MANAGER.with(|ctx| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			if mgr.panel_owners.get(&id) == mgr.current_plugin_id.as_ref()
				&& let Some(panel) = mgr.panels.get_mut(&id)
			{
				panel.focused = focused.0 != 0;
				if panel.focused {
					for (pid, p) in &mut mgr.panels {
						if *pid != id {
							p.focused = false;
						}
					}
				}
			}
		}
	})
}

pub(crate) extern "C" fn host_panel_append_transcript(
	id: TomePanelId,
	role: TomeChatRole,
	text: TomeStr,
) {
	ACTIVE_MANAGER.with(|ctx| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			if mgr.panel_owners.get(&id) == mgr.current_plugin_id.as_ref()
				&& let Some(panel) = mgr.panels.get_mut(&id)
			{
				panel.transcript.push(ChatItem {
					role,
					text: tome_str_to_str(&text).to_string(),
				});
			}
		}
	})
}

pub(crate) extern "C" fn host_request_redraw() {
	ACTIVE_EDITOR.with(|ctx| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			ed.request_redraw();
		}
	})
}

pub(crate) extern "C" fn host_show_message(kind: TomeMessageKind, msg: TomeStr) {
	let s = tome_str_to_str(&msg);
	ACTIVE_EDITOR.with(|ctx| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			match kind {
				TomeMessageKind::Info => ed.show_message(s),
				TomeMessageKind::Error => ed.show_error(s),
			}
		}
	})
}

pub(crate) extern "C" fn host_insert_text(text: TomeStr) {
	let s = tome_str_to_str(&text);
	ACTIVE_EDITOR.with(|ctx| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			ed.insert_text(s);
		}
	})
}

pub(crate) extern "C" fn host_get_current_path(out: *mut TomeOwnedStr) -> TomeStatus {
	ACTIVE_EDITOR.with(|ctx| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &*ed_ptr };

			// The editor swaps `path` and `scratch.path` while executing in scratch context.
			// Prefer the "real" file path when possible.
			let path = if ed.in_scratch_context() {
				ed.scratch.path.as_ref().or(ed.path.as_ref())
			} else {
				ed.path.as_ref().or(ed.scratch.path.as_ref())
			};

			if let Some(path) = path {
				let s = path.to_string_lossy().to_string();
				unsafe { *out = string_to_tome_owned(s) };
				TomeStatus::Ok
			} else {
				TomeStatus::Failed
			}
		} else {
			TomeStatus::AccessDenied
		}
	})
}

pub(crate) extern "C" fn host_free_str(s: TomeOwnedStr) {
	if s.ptr.is_null() {
		return;
	}

	unsafe {
		let slice = std::ptr::slice_from_raw_parts_mut(s.ptr, s.len);
		drop(Box::from_raw(slice));
	}
}

fn string_to_tome_owned(s: String) -> TomeOwnedStr {
	let bytes = s.into_bytes().into_boxed_slice();
	let len = bytes.len();
	let ptr = Box::into_raw(bytes) as *mut u8;
	TomeOwnedStr { ptr, len }
}

pub(crate) extern "C" fn host_fs_read_text(path: TomeStr, out: *mut TomeOwnedStr) -> TomeStatus {
	ACTIVE_EDITOR.with(|ctx| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			let path_str = tome_str_to_str(&path);
			let path_buf = PathBuf::from(path_str);

			// Simplified check: allow if within workspace root (if we had one) or same dir as current file.
			let allowed = if let Some(current_path) = ed.path.as_ref() {
				if let Some(parent) = current_path.parent() {
					path_buf.starts_with(parent)
				} else {
					false
				}
			} else {
				false
			};

			if !allowed {
				// In the future, this should trigger the RequestPermission flow if not headlessly auto-allowed.
				ed.show_error(format!(
					"Plugin tried to read restricted path: {}",
					path_str
				));
				return TomeStatus::AccessDenied;
			}

			match std::fs::read_to_string(path_buf) {
				Ok(content) => {
					unsafe { *out = string_to_tome_owned(content) };
					TomeStatus::Ok
				}
				Err(_) => TomeStatus::Failed,
			}
		} else {
			TomeStatus::AccessDenied
		}
	})
}

pub(crate) extern "C" fn host_fs_write_text(path: TomeStr, content: TomeStr) -> TomeStatus {
	ACTIVE_EDITOR.with(|ctx| {
		if let Some(ed_ptr) = *ctx.borrow() {
			let ed = unsafe { &mut *ed_ptr };
			let path_str = tome_str_to_str(&path);
			let path_buf = PathBuf::from(path_str);
			let content_str = tome_str_to_str(&content);

			// Simplified check
			let allowed = if let Some(current_path) = ed.path.as_ref() {
				if let Some(parent) = current_path.parent() {
					path_buf.starts_with(parent)
				} else {
					false
				}
			} else {
				false
			};

			if !allowed {
				ed.show_error(format!(
					"Plugin tried to write to restricted path: {}",
					path_str
				));
				return TomeStatus::AccessDenied;
			}

			match std::fs::write(path_buf, content_str) {
				Ok(_) => TomeStatus::Ok,
				Err(_) => TomeStatus::Failed,
			}
		} else {
			TomeStatus::AccessDenied
		}
	})
}

pub(crate) extern "C" fn host_register_command(spec: TomeCommandSpecV1) {
	ACTIVE_MANAGER.with(|ctx| {
		if let Some(mgr_ptr) = *ctx.borrow() {
			let mgr = unsafe { &mut *mgr_ptr };
			mgr.register_command(spec);
		}
	})
}
