mod app;
mod backend;
mod capabilities;
mod cli;
mod editor;
mod plugin;
mod render;
mod styles;
mod terminal;
pub mod terminal_panel;
#[cfg(test)]
mod tests;
pub mod theme;
pub mod themes;

use std::io;

use app::run_editor;
use clap::Parser;
use cli::{Cli, Commands, PluginCommands};
use editor::Editor;
use plugin::PluginManager;

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
			editor.start_terminal_prewarm();
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
		PluginCommands::DevAdd { path } => {
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
				let dev_path = path.join("target/debug").join(lib_name);
				manifest.dev_library_path = Some(dev_path.to_string_lossy().to_string());
			}

			let plugin_dir = home::home_dir()
				.map(|h| h.join(".config/tome/plugins").join(&manifest.id))
				.ok_or_else(|| io::Error::other("Could not find home directory"))?;

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
		}
		PluginCommands::Add { from_path } => {
			let manifest_path = from_path.join("plugin.toml");
			let content = std::fs::read_to_string(manifest_path)?;
			let manifest: plugin::manager::PluginManifest = toml::from_str(&content)
				.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

			let plugin_dir = home::home_dir()
				.map(|h| h.join(".config/tome/plugins").join(&manifest.id))
				.ok_or_else(|| io::Error::other("Could not find home directory"))?;

			if plugin_dir.exists() {
				return Err(io::Error::other(format!(
					"Plugin {} already exists at {:?}",
					manifest.id, plugin_dir
				)));
			}

			// Copy the whole directory
			copy_dir(&from_path, &plugin_dir)?;
			println!("Installed plugin: {}", manifest.id);

			if !mgr.config.plugins.enabled.contains(&manifest.id) {
				let id = manifest.id.clone();
				mgr.config.plugins.enabled.push(id.clone());
				mgr.save_config();
				println!("Enabled plugin: {}", id);
			}
		}
		PluginCommands::Remove { id } => {
			let plugin_dir = home::home_dir()
				.map(|h| h.join(".config/tome/plugins").join(&id))
				.ok_or_else(|| io::Error::other("Could not find home directory"))?;

			if plugin_dir.exists() {
				std::fs::remove_dir_all(plugin_dir)?;
				println!("Removed plugin: {}", id);
			} else {
				println!("Plugin {} not found in plugins directory", id);
			}

			mgr.config.plugins.enabled.retain(|e| e != &id);
			mgr.save_config();
		}
		PluginCommands::Enable { id } => {
			if !mgr.config.plugins.enabled.contains(&id) {
				mgr.config.plugins.enabled.push(id.clone());
				mgr.save_config();
				println!("Enabled plugin: {}", id);
			} else {
				println!("Plugin {} is already enabled", id);
			}
		}
		PluginCommands::Disable { id } => {
			if mgr.config.plugins.enabled.contains(&id) {
				mgr.config.plugins.enabled.retain(|e| e != &id);
				mgr.save_config();
				println!("Disabled plugin: {}", id);
			} else {
				println!("Plugin {} is already disabled", id);
			}
		}
		PluginCommands::Reload { id } => {
			println!(
				"Please use :plugins reload {} inside Tome to reload without restarting.",
				id
			);
		}
	}
	Ok(())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> io::Result<()> {
	const IGNORE: &[&str] = &[".git", "target"];
	if let Some(name) = src.file_name().and_then(|s| s.to_str()) {
		if IGNORE.contains(&name) {
			return Ok(());
		}
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
