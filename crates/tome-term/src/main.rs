mod app;
mod backend;
mod capabilities;
mod cli;
mod editor;
mod plugins;
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
use cli::Cli;
use editor::Editor;

fn main() -> io::Result<()> {
	let cli = Cli::parse();

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
