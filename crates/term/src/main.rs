mod app;
mod backend;
mod cli;
mod terminal;
#[cfg(test)]
mod tests;

use app::run_editor;
use clap::Parser;
use cli::{Cli, Command, GrammarAction};
use evildoer_api::Editor;

// Force-link crates to ensure their distributed_slice registrations are included.
#[allow(unused_imports, reason = "linkme distributed_slice registration")]
use {evildoer_acp as _, evildoer_extensions as _, evildoer_stdlib as _};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	// Handle grammar subcommands before starting the editor
	if let Some(Command::Grammar { action }) = cli.command {
		return handle_grammar_command(action);
	}

	// Ensure runtime directory is populated with query files
	if let Err(e) = evildoer_language::ensure_runtime() {
		eprintln!("Warning: failed to seed runtime: {e}");
	}

	// Load themes from runtime directory
	let themes_dir = evildoer_language::runtime_dir().join("themes");
	if let Err(e) = evildoer_config::load_and_register_themes(&themes_dir) {
		eprintln!("Warning: failed to load themes from {:?}: {}", themes_dir, e);
	}

	let mut editor = match cli.file {
		Some(path) => Editor::new(path).await?,
		None => Editor::new_scratch(),
	};

	// Apply theme from CLI if specified
	if let Some(theme_name) = cli.theme
		&& let Err(e) = editor.set_theme(&theme_name)
	{
		eprintln!("Warning: failed to set theme '{}': {}", theme_name, e);
	}

	run_editor(editor).await?;
	Ok(())
}

fn handle_grammar_command(action: GrammarAction) -> anyhow::Result<()> {
	use evildoer_language::build::{build_all_grammars, fetch_all_grammars, load_grammar_configs};

	let configs = load_grammar_configs()?;

	match action {
		GrammarAction::Fetch { only } => {
			let configs: Vec<_> = if let Some(ref names) = only {
				configs
					.into_iter()
					.filter(|c| names.contains(&c.grammar_id))
					.collect()
			} else {
				configs
			};
			println!("Fetching {} grammars...", configs.len());
			let results = fetch_all_grammars(configs, None);
			report_fetch_results(&results);
		}
		GrammarAction::Build { only } => {
			let configs: Vec<_> = if let Some(ref names) = only {
				configs
					.into_iter()
					.filter(|c| names.contains(&c.grammar_id))
					.collect()
			} else {
				configs
			};
			println!("Building {} grammars...", configs.len());
			let results = build_all_grammars(configs, None);
			report_build_results(&results);
		}
		GrammarAction::Sync { only } => {
			let configs: Vec<_> = if let Some(ref names) = only {
				configs
					.into_iter()
					.filter(|c| names.contains(&c.grammar_id))
					.collect()
			} else {
				configs
			};
			println!("Syncing {} grammars...", configs.len());
			println!("\n=== Fetching ===");
			let fetch_results = fetch_all_grammars(configs.clone(), None);
			report_fetch_results(&fetch_results);
			println!("\n=== Building ===");
			let build_results = build_all_grammars(configs, None);
			report_build_results(&build_results);
		}
	}

	Ok(())
}

fn report_fetch_results(
	results: &[(
		evildoer_language::build::GrammarConfig,
		Result<evildoer_language::build::FetchStatus, evildoer_language::build::GrammarBuildError>,
	)],
) {
	use evildoer_language::build::FetchStatus;
	let mut success = 0;
	let mut skipped = 0;
	let mut failed = 0;

	for (config, result) in results {
		let name = &config.grammar_id;
		match result {
			Ok(FetchStatus::Updated) => {
				println!("  ✓ {name} (updated)");
				success += 1;
			}
			Ok(FetchStatus::UpToDate) => {
				println!("  - {name} (up to date)");
				skipped += 1;
			}
			Ok(FetchStatus::Local) => {
				println!("  - {name} (local)");
				skipped += 1;
			}
			Err(e) => {
				eprintln!("  ✗ {name}: {e}");
				failed += 1;
			}
		}
	}

	println!("\nFetch: {success} succeeded, {skipped} skipped, {failed} failed");
}

fn report_build_results(
	results: &[(
		evildoer_language::build::GrammarConfig,
		Result<evildoer_language::build::BuildStatus, evildoer_language::build::GrammarBuildError>,
	)],
) {
	use evildoer_language::build::BuildStatus;
	let mut success = 0;
	let mut skipped = 0;
	let mut failed = 0;

	for (config, result) in results {
		let name = &config.grammar_id;
		match result {
			Ok(BuildStatus::Built) => {
				println!("  ✓ {name}");
				success += 1;
			}
			Ok(BuildStatus::AlreadyBuilt) => {
				println!("  - {name} (up to date)");
				skipped += 1;
			}
			Err(e) => {
				eprintln!("  ✗ {name}: {e}");
				failed += 1;
			}
		}
	}

	println!("\nBuild: {success} succeeded, {skipped} skipped, {failed} failed");
}
