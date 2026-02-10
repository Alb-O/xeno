//! Background task spawning helpers.
//!
//! These methods spawn fire-and-forget tasks that send [`crate::msg::EditorMsg`] on completion.
//! The main loop drains messages and applies them to editor state.

use std::path::PathBuf;

use super::Editor;
#[cfg(feature = "lsp")]
use crate::msg::LspMsg;
use crate::msg::{EditorMsg, IoMsg, MsgSender, ThemeMsg};
use crate::paste::normalize_to_lf;

impl Editor {
	/// Spawns a background task to load and register themes.
	///
	/// Seeds embedded themes into the data directory, then loads from
	/// embedded, data, and config directories. Later sources override earlier
	/// by ID: config > data > embedded. Sends [`crate::msg::ThemeMsg::ThemesReady`] when
	/// complete.
	pub fn kick_theme_load(&self) {
		let tx = self.msg_tx();
		let config_themes_dir = crate::paths::get_config_dir().map(|d| d.join("themes"));
		let data_themes_dir = crate::paths::get_data_dir().map(|d| d.join("themes"));

		tokio::spawn(async move {
			let errors = load_themes_blocking(config_themes_dir, data_themes_dir).await;
			send(&tx, ThemeMsg::ThemesReady { errors });
		});
	}

	/// Spawns a background task to load a file.
	///
	/// Sends [`crate::msg::IoMsg::FileLoaded`] or [`crate::msg::IoMsg::LoadFailed`] on completion.
	pub fn kick_file_load(&self, path: PathBuf) {
		let tx = self.msg_tx();
		tokio::spawn(async move {
			match tokio::fs::read_to_string(&path).await {
				Ok(content) => {
					let rope = ropey::Rope::from_str(&normalize_to_lf(content));
					let readonly = !is_writable(&path);
					send(
						&tx,
						IoMsg::FileLoaded {
							path,
							rope,
							readonly,
						},
					);
				}
				Err(error) => {
					send(&tx, IoMsg::LoadFailed { path, error });
				}
			}
		});
	}

	/// Spawns a background task to load LSP server configurations.
	///
	/// Parses `lsp.kdl` and `languages.kdl`, registers server configs with the
	/// registry. Server availability is tested at spawn time rather than upfront.
	/// Sends [`LspMsg::CatalogReady`] when complete.
	#[cfg(feature = "lsp")]
	pub fn kick_lsp_catalog_load(&self) {
		let sync = self.state.lsp.sync_clone();
		let tx = self.msg_tx();

		tokio::spawn(async move {
			let server_defs = match xeno_runtime_language::load_lsp_configs() {
				Ok(defs) => defs,
				Err(e) => {
					tracing::warn!(error = %e, "Failed to load LSP configs");
					send(&tx, LspMsg::CatalogReady);
					return;
				}
			};
			let lang_mapping = xeno_runtime_language::language_db().lsp_mapping();

			let server_map: std::collections::HashMap<_, _> =
				server_defs.iter().map(|s| (s.name.as_str(), s)).collect();

			for (language, info) in &lang_mapping {
				let Some(server_def) = info
					.servers
					.iter()
					.find_map(|name| server_map.get(name.as_str()))
				else {
					continue;
				};

				sync.registry().register(
					language.clone(),
					xeno_lsp::LanguageServerConfig {
						command: server_def.command.clone(),
						args: server_def.args.clone(),
						env: server_def.environment.clone(),
						root_markers: info.roots.clone(),
						config: server_def.config.clone(),
						..Default::default()
					},
				);
			}

			tracing::debug!(languages = lang_mapping.len(), "LSP catalog loaded");
			send(&tx, LspMsg::CatalogReady);
		});
	}

	#[cfg(not(feature = "lsp"))]
	pub fn kick_lsp_catalog_load(&self) {}
}

/// Loads and registers all themes in a single batch.
///
/// Override order (later entries shadow earlier by ID):
/// 1. Embedded themes from the binary
/// 2. Data-directory themes (`~/.local/share/xeno/themes/`)
/// 3. Config-directory themes (`~/.config/xeno/themes/`)
///
/// Embedded themes are seeded into the data directory before loading so users
/// can discover and customize them on disk.
async fn load_themes_blocking(
	config_themes_dir: Option<PathBuf>,
	data_themes_dir: Option<PathBuf>,
) -> Vec<(String, String)> {
	tokio::task::spawn_blocking(move || {
		let mut errors = Vec::new();
		let mut all_themes: Vec<xeno_registry::themes::LinkedThemeDef> = Vec::new();

		if let Some(ref dir) = data_themes_dir {
			collect_dir_themes(dir, &mut all_themes, &mut errors);
		}

		if let Some(ref dir) = config_themes_dir {
			collect_dir_themes(dir, &mut all_themes, &mut errors);
		}

		// Deduplicate by canonical ID (xeno-registry::<name>)
		let mut deduped = std::collections::HashMap::new();
		for theme in all_themes {
			deduped.insert(theme.meta.id.clone(), theme);
		}

		xeno_registry::themes::register_runtime_themes(deduped.into_values().collect());

		errors
	})
	.await
	.unwrap_or_default()
}

/// Loads themes from `dir` into the accumulator vectors, logging on failure.
fn collect_dir_themes(
	dir: &std::path::Path,
	themes: &mut Vec<xeno_registry::themes::LinkedThemeDef>,
	errors: &mut Vec<(String, String)>,
) {
	use xeno_registry::config::kdl::parse_theme_standalone_str;

	if !dir.exists() {
		return;
	}

	let entries = match std::fs::read_dir(dir) {
		Ok(e) => e,
		Err(e) => {
			tracing::warn!(dir = %dir.display(), error = %e, "failed to read themes directory");
			return;
		}
	};

	for entry in entries.flatten() {
		let path = entry.path();
		if path.extension().is_some_and(|ext| ext == "kdl") {
			let filename = path
				.file_name()
				.unwrap_or_default()
				.to_string_lossy()
				.into_owned();

			match std::fs::read_to_string(&path) {
				Ok(content) => match parse_theme_standalone_str(&content) {
					Ok(theme) => themes.push(theme),
					Err(e) => errors.push((filename, e.to_string())),
				},
				Err(e) => {
					errors.push((filename, e.to_string()));
				}
			}
		}
	}
}

fn send<M: Into<EditorMsg>>(tx: &MsgSender, msg: M) {
	let _ = tx.send(msg.into());
}

fn is_writable(path: &std::path::Path) -> bool {
	std::fs::OpenOptions::new().write(true).open(path).is_ok()
}
