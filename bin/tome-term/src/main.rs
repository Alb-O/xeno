mod app;
mod backend;
mod capabilities;
mod cli;
mod editor;
mod ipc;
mod plugin;
mod render;
mod styles;
mod terminal;
pub mod terminal_panel;
#[cfg(test)]
mod tests;
pub mod theme;
pub mod themes;
mod ui;

use std::io;
use std::path::PathBuf;

use app::run_editor;
use clap::Parser;
use cli::{Cli, Commands, PluginCommands};
use editor::Editor;
use plugin::manager::{PluginManager, get_plugins_dir};

fn main() -> io::Result<()> {
	let cli = Cli::parse();

	if let Some(cmd) = cli.command {
		match cmd {
			Commands::Plugin(args) => {
				return handle_plugin_command(args.command);
			}
		}
	}

	let mut editor = match cli.file {
		Some(path) => Editor::new(path)?,
		None => Editor::new_scratch(),
	};

	if cli.quit_after_ex {
		if let Some(cmd) = cli.ex.as_deref() {
			editor.ui_startup();
			editor.autoload_plugins();
			editor.execute_ex_command(cmd);

			// Give plugins time to emit async errors/messages.
			// Print messages as they arrive so failures are visible headlessly.
			let mut last_text: Option<String> = None;
			for _ in 0..500 {
				editor.poll_plugins();

				if let Some(message) = &editor.message
					&& last_text.as_deref() != Some(message.text.as_str())
				{
					match message.kind {
						editor::MessageKind::Info => {
							eprintln!("{}", message.text);
							if message.text.starts_with("Failed to start agent:")
								|| message.text.starts_with("ACP IO error:")
								|| message.text.starts_with("ACP initialize failed:")
								|| message.text.starts_with("ACP new_session failed:")
							{
								return Err(io::Error::other(message.text.clone()));
							}
						}
						editor::MessageKind::Error => {
							return Err(io::Error::other(message.text.clone()));
						}
					}
					last_text = Some(message.text.clone());
				}

				std::thread::sleep(std::time::Duration::from_millis(20));
			}
		}
		return Ok(());
	}

	run_editor(editor, cli.ex, false)
}

fn handle_plugin_command(cmd: PluginCommands) -> io::Result<()> {
	let mut mgr = PluginManager::new();
	mgr.discover_plugins();
	mgr.load_config();

	match cmd {
		PluginCommands::Add { path, dev } => {
			if dev {
				let path = std::fs::canonicalize(&path)?;
				let manifest_path = path.join("plugin.toml");
				if !manifest_path.exists() {
					return Err(io::Error::new(
						io::ErrorKind::NotFound,
						format!("plugin.toml not found in {:?}", path),
					));
				}
				let content = std::fs::read_to_string(&manifest_path)?;
				let mut manifest = toml::from_str::<plugin::manager::PluginManifest>(&content)
					.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

				// Guess the dev library path if not provided
				if manifest.dev_library_path.is_none() {
					let prefix = std::env::consts::DLL_PREFIX;
					let ext = std::env::consts::DLL_EXTENSION;
					let lib_name = format!(
						"{}tome_{}_plugin.{}",
						prefix,
						manifest.id.replace('-', "_"),
						ext
					);
					// Try workspace root first (guessing it's parent of plugins/)
					let mut dev_path = path.join("target/debug").join(&lib_name);
					if !dev_path.exists() {
						// Maybe it's a workspace and we are in plugins/
						if let Some(parent) = path.parent()
							&& parent.file_name().and_then(|n| n.to_str()) == Some("plugins")
							&& let Some(workspace_root) = parent.parent()
						{
							let ws_dev_path = workspace_root.join("target/debug").join(&lib_name);
							if ws_dev_path.exists() {
								dev_path = ws_dev_path;
							}
						}
					}
					manifest.dev_library_path = Some(dev_path.to_string_lossy().to_string());
				} else {
					// Canonicalize existing dev_library_path if it's relative to the crate
					let dev_path_str = manifest.dev_library_path.as_ref().unwrap();
					let dev_path = PathBuf::from(dev_path_str);
					if dev_path.is_relative() {
						let abs_dev_path = path.join(dev_path);
						manifest.dev_library_path =
							Some(abs_dev_path.to_string_lossy().to_string());
					}
				}

				let plugin_dir = get_plugins_dir()
					.map(|d| d.join(&manifest.id))
					.ok_or_else(|| io::Error::other("Could not resolve plugins directory"))?;

				std::fs::create_dir_all(&plugin_dir)?;
				let new_manifest_content = toml::to_string(&manifest)
					.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
				std::fs::write(plugin_dir.join("plugin.toml"), new_manifest_content)?;

				println!("Registered dev plugin: {}", manifest.id);
				if !mgr.config.plugins.enabled.contains(&manifest.id) {
					let id = manifest.id.clone();
					mgr.config.plugins.enabled.push(id.clone());
					mgr.save_config();
					println!("Enabled plugin: {}", id);
				}
			} else {
				let manifest_path = path.join("plugin.toml");
				let content = std::fs::read_to_string(manifest_path)?;
				let manifest: plugin::manager::PluginManifest = toml::from_str(&content)
					.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

				let plugin_dir = get_plugins_dir()
					.map(|d| d.join(&manifest.id))
					.ok_or_else(|| io::Error::other("Could not resolve plugins directory"))?;

				if plugin_dir.exists() {
					return Err(io::Error::other(format!(
						"Plugin {} already exists at {:?}",
						manifest.id, plugin_dir
					)));
				}

				// Copy the entire plugin directory recursively
				copy_dir(&path, &plugin_dir)?;
				println!("Installed plugin: {}", manifest.id);

				if !mgr.config.plugins.enabled.contains(&manifest.id) {
					let id = manifest.id.clone();
					mgr.config.plugins.enabled.push(id.clone());
					mgr.save_config();
					println!("Enabled plugin: {}", id);
				}
			}
		}
		PluginCommands::List => {
			let mut ids: Vec<_> = mgr.entries.keys().collect();
			ids.sort();

			if ids.is_empty() {
				println!("No plugins installed.");
				return Ok(());
			}

			let mut rows = Vec::new();
			rows.push((
				"ID".to_string(),
				"VERSION".to_string(),
				"STATUS".to_string(),
				"NAME".to_string(),
			));

			for id in &ids {
				let entry = &mgr.entries[*id];
				let status = if mgr.config.plugins.enabled.contains(*id) {
					"Enabled"
				} else {
					"Disabled"
				};
				rows.push((
					id.to_string(),
					entry.manifest.version.clone(),
					status.to_string(),
					entry.manifest.name.clone(),
				));
			}

			let w_id = rows.iter().map(|r| r.0.len()).max().unwrap_or(0);
			let w_ver = rows.iter().map(|r| r.1.len()).max().unwrap_or(0);
			let w_stat = rows.iter().map(|r| r.2.len()).max().unwrap_or(0);

			use clap::builder::styling::{AnsiColor, Effects};
			let header_style = AnsiColor::Green.on_default().effects(Effects::BOLD);
			let enabled_style = AnsiColor::Green.on_default();
			let disabled_style = AnsiColor::Yellow.on_default();

			for (i, (id, ver, stat, name)) in rows.into_iter().enumerate() {
				if i == 0 {
					println!(
						"{}{: <id_w$} {: <ver_w$} {: <stat_w$} {}{}",
						header_style.render(),
						id,
						ver,
						stat,
						name,
						header_style.render_reset(),
						id_w = w_id,
						ver_w = w_ver,
						stat_w = w_stat,
					);
					continue;
				}

				let s_style = if stat == "Enabled" {
					enabled_style
				} else {
					disabled_style
				};

				println!(
					"{: <id_w$} {: <ver_w$} {}{:<stat_w$}{} {}",
					id,
					ver,
					s_style.render(),
					stat,
					s_style.render_reset(),
					name,
					id_w = w_id,
					ver_w = w_ver,
					stat_w = w_stat,
				);
			}
		}
		PluginCommands::Remove { ids } => {
			for id in ids {
				let plugin_dir = get_plugins_dir()
					.map(|d| d.join(&id))
					.ok_or_else(|| io::Error::other("Could not resolve plugins directory"))?;

				if plugin_dir.exists() {
					std::fs::remove_dir_all(plugin_dir)?;
					println!("Removed plugin: {}", id);
				} else {
					println!("Plugin {} not found in plugins directory", id);
				}

				mgr.config.plugins.enabled.retain(|e| e != &id);
			}
			mgr.save_config();
		}
		PluginCommands::Enable { ids } => {
			for id in ids {
				if !mgr.config.plugins.enabled.contains(&id) {
					mgr.config.plugins.enabled.push(id.clone());
					println!("Enabled plugin: {}", id);
				} else {
					println!("Plugin {} is already enabled", id);
				}
			}
			mgr.save_config();
		}
		PluginCommands::Disable { ids } => {
			for id in ids {
				if mgr.config.plugins.enabled.contains(&id) {
					mgr.config.plugins.enabled.retain(|e| e != &id);
					println!("Disabled plugin: {}", id);
				} else {
					println!("Plugin {} is already disabled", id);
				}
			}
			mgr.save_config();
		}
		PluginCommands::Reload { ids } => {
			let rt = tokio::runtime::Builder::new_current_thread()
				.enable_all()
				.build()?;
			for id in ids {
				match rt.block_on(crate::ipc::send_client_msg(&format!("reload {}", id))) {
					Ok(_) => println!("Sent reload command for plugin {}", id),
					Err(e) => {
						println!("Failed to send reload command for {}: {}", id, e);
						println!("Is Tome running?");
						println!("(You can also use :plugins reload {} inside Tome)", id);
					}
				}
			}
		}
	}
	Ok(())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> io::Result<()> {
	const IGNORE: &[&str] = &[".git", "target"];
	if let Some(name) = src.file_name().and_then(|s| s.to_str())
		&& IGNORE.contains(&name)
	{
		return Ok(());
	}

	std::fs::create_dir_all(dst)?;
	for entry in std::fs::read_dir(src)? {
		let entry = entry?;
		let path = entry.path();
		let name = entry.file_name();
		let name_str = name.to_string_lossy();

		if IGNORE.contains(&name_str.as_ref()) {
			continue;
		}

		let ty = entry.file_type()?;
		if ty.is_dir() {
			copy_dir(&path, &dst.join(name))?;
		} else {
			std::fs::copy(path, dst.join(name))?;
		}
	}
	Ok(())
}
