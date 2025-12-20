use std::collections::HashMap;
use std::path::PathBuf;

use libloading::{Library, Symbol};
use tome_cabi_types::{
	TOME_C_ABI_VERSION_V2, TomeCommandContextV1, TomeCommandSpecV1, TomeGuestV2, TomePluginEntryV2,
	TomeStatus,
};

use crate::editor::Editor;
use crate::plugin::panels::ChatPanelState;

pub mod context;
pub mod host;
pub mod types;

pub use context::{ACTIVE_EDITOR, ACTIVE_MANAGER, PluginContextGuard};
pub use host::{HOST_V2, tome_owned_to_string};
pub use types::{
	PendingPermission, PluginEntry, PluginError, PluginManifest, PluginStatus, TomeConfig,
};

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
	pub panel_owners: HashMap<u64, String>,
	pub panel_ui_ids: HashMap<u64, String>,
	pub logs: HashMap<String, Vec<String>>,
	pub(crate) next_panel_id: u64,
	pub(crate) current_namespace: Option<String>,
	pub(crate) current_plugin_id: Option<String>,
}

pub fn get_config_dir() -> Option<PathBuf> {
	if let Ok(dir) = std::env::var("TOME_CONFIG_DIR") {
		return Some(PathBuf::from(dir));
	}
	home::home_dir().map(|h| h.join(".config/tome"))
}

pub fn get_plugins_dir() -> Option<PathBuf> {
	get_config_dir().map(|d| d.join("plugins"))
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
			panel_ui_ids: HashMap::new(),
			logs: HashMap::new(),
			next_panel_id: 1,
			current_namespace: None,
			current_plugin_id: None,
		}
	}

	pub fn load(&mut self, ed: &mut Editor, id: &str) -> Result<(), PluginError> {
		let lib_path = {
			let entry = self
				.entries
				.get(id)
				.ok_or_else(|| PluginError::NotFound(id.to_string()))?;

			if entry.manifest.abi != TOME_C_ABI_VERSION_V2 {
				return Err(PluginError::AbiMismatch {
					id: id.to_string(),
					expected: TOME_C_ABI_VERSION_V2,
					actual: entry.manifest.abi,
				});
			}

			if let Some(dev_path) = &entry.manifest.dev_library_path {
				let p = PathBuf::from(dev_path);
				if p.is_absolute() {
					p
				} else {
					entry.path.join(p)
				}
			} else if let Some(lib_name) = &entry.manifest.library_path {
				entry.path.join(lib_name)
			} else {
				return Err(PluginError::MissingLibraryPath(id.to_string()));
			}
		};

		if !lib_path.exists() {
			return Err(PluginError::LibraryNotFound(lib_path));
		}

		unsafe {
			let lib = Library::new(&lib_path).map_err(|e| PluginError::Lib(e.to_string()))?;
			let entry_point: Symbol<TomePluginEntryV2> = lib
				.get(b"tome_plugin_entry_v2")
				.map_err(|e| PluginError::Lib(e.to_string()))?;

			let mut guest = std::mem::MaybeUninit::<TomeGuestV2>::uninit();
			let status = entry_point(&HOST_V2, guest.as_mut_ptr());
			if status != TomeStatus::Ok {
				return Err(PluginError::EntryPointFailed(status));
			}
			let guest = guest.assume_init();

			if guest.abi_version != TOME_C_ABI_VERSION_V2 {
				return Err(PluginError::AbiMismatch {
					id: id.to_string(),
					expected: TOME_C_ABI_VERSION_V2,
					actual: guest.abi_version,
				});
			}

			self.current_plugin_id = Some(id.to_string());
			self.current_namespace = Some(id.to_string());

			if let Some(init) = guest.init {
				let ed_ptr = ed as *mut Editor;
				let mgr_ptr = self as *mut PluginManager;
				let _guard = PluginContextGuard::new(mgr_ptr, ed_ptr, id);
				let status = init(&HOST_V2);
				if status != TomeStatus::Ok {
					return Err(PluginError::InitFailed(id.to_string(), status));
				}
			}

			self.current_plugin_id = None;
			self.current_namespace = None;

			self.plugins.insert(
				id.to_string(),
				LoadedPlugin {
					lib,
					guest,
					id: id.to_string(),
				},
			);

			if let Some(entry) = self.entries.get_mut(id) {
				entry.status = PluginStatus::Loaded;
			}
		}

		Ok(())
	}

	pub fn discover_plugins(&mut self) {
		let dirs = vec![
			std::env::var("TOME_PLUGIN_DIR").ok().map(PathBuf::from),
			get_plugins_dir(),
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
		let config_path = get_config_dir().map(|d| d.join("config.toml"));
		if let Some(path) = config_path
			&& path.exists()
			&& let Ok(content) = std::fs::read_to_string(path)
			&& let Ok(config) = toml::from_str::<TomeConfig>(&content)
		{
			self.config = config;
		}
	}

	pub fn save_config(&mut self) {
		if let Some(dir) = get_config_dir() {
			if !dir.exists()
				&& let Err(e) = std::fs::create_dir_all(&dir)
			{
				eprintln!("Failed to create config directory {:?}: {}", dir, e);
				return;
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
			return;
		}

		for (id, entry) in &mut self.entries {
			if !self.config.plugins.enabled.contains(id) {
				entry.status = PluginStatus::Disabled;
			}
		}

		let enabled = self.config.plugins.enabled.clone();
		for id in enabled {
			if let Err(e) = self.load(ed, &id) {
				eprintln!("Failed to load plugin {}: {}", id, e);
				self.logs
					.entry(id.clone())
					.or_default()
					.push(format!("ERROR: Failed to load: {}", e));
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
			let name = host::tome_str_to_str(&spec.name).to_string();
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

			if !spec.aliases.ptr.is_null() && spec.aliases.len > 0 {
				let aliases =
					unsafe { std::slice::from_raw_parts(spec.aliases.ptr, spec.aliases.len) };
				for alias in aliases {
					let alias_name = host::tome_str_to_str(alias).to_string();
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

impl Default for PluginManager {
	fn default() -> Self {
		Self::new()
	}
}
