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
	/// Spawns a background task to load themes from disk.
	///
	/// Collects and deduplicates themes from data and config directories.
	/// Registration is deferred to the editor thread via [`ThemeMsg::ThemesReady`]
	/// to avoid races when multiple loads overlap.
	pub fn kick_theme_load(&mut self) {
		let token = self.state.async_state.theme_load_token_next;
		self.state.async_state.theme_load_token_next += 1;
		self.state.async_state.pending_theme_load_token = Some(token);

		let tx = self.msg_tx();
		let config_themes_dir = crate::paths::get_config_dir().map(|d| d.join("themes"));
		let data_themes_dir = crate::paths::get_data_dir().map(|d| d.join("themes"));
		xeno_worker::spawn(xeno_worker::TaskClass::Background, async move {
			let (themes, errors) = load_themes_blocking(config_themes_dir, data_themes_dir).await;
			send(&tx, ThemeMsg::ThemesReady { token, themes, errors });
		});
	}

	/// Spawns a background task to load a file.
	///
	/// The `token` is a monotonic ID carried through to the completion message
	/// so that stale loads (superseded by a newer request) can be detected.
	/// Sends [`crate::msg::IoMsg::FileLoaded`] or [`crate::msg::IoMsg::LoadFailed`] on completion.
	pub fn kick_file_load(&self, path: PathBuf, token: u64) {
		let tx = self.msg_tx();
		xeno_worker::spawn(xeno_worker::TaskClass::IoBlocking, async move {
			match tokio::fs::read_to_string(&path).await {
				Ok(content) => {
					let path_for_build = path.clone();
					let built = xeno_worker::spawn_blocking(xeno_worker::TaskClass::CpuBlocking, move || {
							let rope = ropey::Rope::from_str(&normalize_to_lf(content));
							let readonly = !is_writable(&path_for_build);
							(rope, readonly)
						})
						.await;

					match built {
						Ok((rope, readonly)) => {
							send(&tx, IoMsg::FileLoaded { path, rope, readonly, token });
						}
						Err(e) => {
							send(
								&tx,
								IoMsg::LoadFailed {
									path,
									error: std::io::Error::other(e.to_string()),
									token,
								},
							);
						}
					}
				}
				Err(error) => {
					send(&tx, IoMsg::LoadFailed { path, error, token });
				}
			}
		});
	}

	/// Spawns a background task to load LSP server configurations.
	///
	/// Reads the immutable registry-backed language/server catalog in a blocking
	/// task and builds a list of `(language, config)` pairs. Registration is deferred
	/// to the editor thread via [`LspMsg::CatalogReady`] to avoid races when
	/// multiple loads overlap.
	#[cfg(feature = "lsp")]
	pub fn kick_lsp_catalog_load(&mut self) {
		let token = self.state.async_state.lsp_catalog_load_token_next;
		self.state.async_state.lsp_catalog_load_token_next += 1;
		self.state.async_state.pending_lsp_catalog_load_token = Some(token);

		let tx = self.msg_tx();

		xeno_worker::spawn(xeno_worker::TaskClass::Background, async move {
			let parsed = xeno_worker::spawn_blocking(xeno_worker::TaskClass::IoBlocking, || {
					xeno_language::load_resolved_lsp_configs().map_err(|e| format!("failed to load LSP configs: {e}"))
				})
				.await;

			let resolved = match parsed {
				Ok(Ok(configs)) => configs,
				Ok(Err(error)) => {
					tracing::warn!(error = %error, "Failed to load LSP configs");
					send(&tx, LspMsg::CatalogReady { token, configs: vec![] });
					return;
				}
				Err(error) => {
					tracing::warn!(error = %error, "Failed to join LSP catalog loader");
					send(&tx, LspMsg::CatalogReady { token, configs: vec![] });
					return;
				}
			};

			let mut configs = Vec::with_capacity(resolved.len());
			for entry in resolved {
				configs.push((
					entry.language,
					xeno_lsp::LanguageServerConfig {
						command: entry.server.command,
						args: entry.server.args,
						env: entry.server.environment,
						root_markers: entry.roots,
						config: entry.server.config,
						..Default::default()
					},
				));
			}

			tracing::debug!(languages = configs.len(), "LSP catalog loaded");
			send(&tx, LspMsg::CatalogReady { token, configs });
		});
	}

	#[cfg(not(feature = "lsp"))]
	pub fn kick_lsp_catalog_load(&mut self) {}
}

/// Loads and deduplicates all themes from disk without registering them.
///
/// Override order (later entries shadow earlier by ID):
/// 1. Data-directory themes (`~/.local/share/xeno/themes/`)
/// 2. Config-directory themes (`~/.config/xeno/themes/`)
///
/// Returns the deduped theme list and any parse errors. Registration happens
/// on the editor thread after token validation.
async fn load_themes_blocking(
	config_themes_dir: Option<PathBuf>,
	data_themes_dir: Option<PathBuf>,
) -> (Vec<xeno_registry::themes::LinkedThemeDef>, Vec<(String, String)>) {
	xeno_worker::spawn_blocking(xeno_worker::TaskClass::IoBlocking, move || {
			let mut errors = Vec::new();
			let mut all_themes: Vec<xeno_registry::themes::LinkedThemeDef> = Vec::new();

			if let Some(ref dir) = data_themes_dir {
				collect_dir_themes(dir, &mut all_themes, &mut errors);
			}

			if let Some(ref dir) = config_themes_dir {
				collect_dir_themes(dir, &mut all_themes, &mut errors);
			}

			// Deduplicate by canonical ID (xeno-registry::<name>)
			let mut deduped = std::collections::BTreeMap::new();
			for theme in all_themes {
				deduped.insert(theme.meta.id.clone(), theme);
			}

			(deduped.into_values().collect(), errors)
		})
		.await
		.unwrap_or_else(|_| (Vec::new(), Vec::new()))
}

/// Loads themes from `dir` into the accumulator vectors, logging on failure.
fn collect_dir_themes(dir: &std::path::Path, themes: &mut Vec<xeno_registry::themes::LinkedThemeDef>, errors: &mut Vec<(String, String)>) {
	use xeno_registry::config::nuon::parse_theme_standalone_str as parse_nuon_theme;

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

	let mut files: Vec<std::path::PathBuf> = entries
		.flatten()
		.map(|entry| entry.path())
		.filter(|path| path.extension().is_some_and(|ext| ext == "nuon"))
		.collect();

	files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

	for path in files {
		let filename = path.file_name().unwrap_or_default().to_string_lossy().into_owned();

		let content = match std::fs::read_to_string(&path) {
			Ok(content) => content,
			Err(e) => {
				errors.push((filename, e.to_string()));
				continue;
			}
		};

		match parse_nuon_theme(&content) {
			Ok(theme) => themes.push(theme),
			Err(e) => errors.push((filename, e.to_string())),
		}
	}
}

fn send<M: Into<EditorMsg>>(tx: &MsgSender, msg: M) {
	let _ = tx.send(msg.into());
}

fn is_writable(path: &std::path::Path) -> bool {
	std::fs::OpenOptions::new().write(true).open(path).is_ok()
}
