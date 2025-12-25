mod app;
mod backend;
mod cli;
mod terminal;
#[cfg(test)]
mod tests;

use app::run_editor;
use clap::Parser;
use cli::Cli;
use tome_api::Editor;
// Force linking of tome-extensions so distributed_slices are registered
#[allow(unused_imports, reason = "ensures tome-extensions distributed_slices are linked")]
use tome_extensions as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	let mut editor = match cli.file {
		Some(path) => Editor::new(path).await?,
		None => Editor::new_scratch(),
	};

	if cli.quit_after_ex {
		if let Some(cmd) = cli.ex.as_deref() {
			editor.ui_startup();
			editor.execute_ex_command(cmd).await;

			// Give async events time to process.
			let mut last_text: Option<String> = None;
			for _ in 0..500 {
				editor.ui_tick();

				if let Some(message) = &editor.message
					&& last_text.as_deref() != Some(message.text.as_str())
				{
					match message.kind {
						tome_api::editor::MessageKind::Info
						| tome_api::editor::MessageKind::Warning => {
							eprintln!("{}", message.text);
							if message.text.starts_with("Failed to start agent:")
								|| message.text.starts_with("ACP IO error:")
								|| message.text.starts_with("ACP initialize failed:")
								|| message.text.starts_with("ACP new_session failed:")
							{
								return Err(anyhow::anyhow!(message.text.clone()));
							}
						}
						tome_api::editor::MessageKind::Error => {
							return Err(anyhow::anyhow!(message.text.clone()));
						}
					}
					last_text = Some(message.text.clone());
				}

				std::thread::sleep(std::time::Duration::from_millis(20));
			}
		}
		return Ok(());
	}

	run_editor(editor, cli.ex, false).await?;
	Ok(())
}
