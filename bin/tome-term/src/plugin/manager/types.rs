use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tome_cabi_types::TomeStatus;

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
	_Failed(String),
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

pub struct PendingPermission {
	pub plugin_id: String,
	pub request_id: u64,
	pub _prompt: String,
	pub _options: Vec<(String, String)>, // (option_id, display_label)
}

#[derive(thiserror::Error, Debug)]
pub enum PluginError {
	#[error("Plugin {0} not found")]
	NotFound(String),
	#[error("ABI version mismatch for plugin {id}: expected {expected}, got {actual}")]
	AbiMismatch {
		id: String,
		expected: u32,
		actual: u32,
	},
	#[error("Library error: {0}")]
	Lib(String),
	#[error("Plugin {0} has no library_path or dev_library_path")]
	MissingLibraryPath(String),
	#[error("Library not found at {0:?}")]
	LibraryNotFound(PathBuf),
	#[error("Entry point failed with status {0:?}")]
	EntryPointFailed(TomeStatus),
	#[error("Plugin {0} init failed with status {1:?}")]
	InitFailed(String, TomeStatus),
}
