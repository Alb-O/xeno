//! Xeno terminal application entry point.

/// Application lifecycle and event loop.
mod app;
/// Terminal backend abstraction.
mod backend;
/// Command-line interface definitions.
mod cli;
mod terminal;
#[cfg(test)]
mod tests;

use app::run_editor;
use clap::Parser;
use cli::{Cli, Command, GrammarAction};
use xeno_api::Editor;
// Force-link crates to ensure their distributed_slice registrations are included.
#[allow(unused_imports, reason = "linkme distributed_slice registration")]
use {xeno_acp as _, xeno_core as _, xeno_extensions as _};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	// Handle grammar subcommands before starting the editor
	if let Some(Command::Grammar { action }) = cli.command {
		return handle_grammar_command(action);
	}

	// Ensure runtime directory is populated with query files
	if let Err(e) = xeno_language::ensure_runtime() {
		eprintln!("Warning: failed to seed runtime: {e}");
	}

	// Load themes from runtime directory
	let themes_dir = xeno_language::runtime_dir().join("themes");
	if let Err(e) = xeno_config::load_and_register_themes(&themes_dir) {
		eprintln!(
			"Warning: failed to load themes from {:?}: {}",
			themes_dir, e
		);
	}

	let mut editor = match cli.file {
		Some(path) => Editor::new(path).await?,
		None => Editor::new_scratch(),
	};

	if let Some(theme_name) = cli.theme
		&& let Err(e) = editor.set_theme(&theme_name)
	{
		eprintln!("Warning: failed to set theme '{}': {}", theme_name, e);
	}

	run_editor(editor).await?;
	Ok(())
}

/// Handles grammar fetch/build/sync subcommands.
fn handle_grammar_command(action: GrammarAction) -> anyhow::Result<()> {
	use xeno_language::build::{build_all_grammars, fetch_all_grammars, load_grammar_configs};

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

/// Prints a summary of grammar fetch results to stdout.
fn report_fetch_results(
	results: &[(
		xeno_language::build::GrammarConfig,
		Result<xeno_language::build::FetchStatus, xeno_language::build::GrammarBuildError>,
	)],
) {
	use xeno_language::build::FetchStatus;
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

/// Prints a summary of grammar build results to stdout.
fn report_build_results(
	results: &[(
		xeno_language::build::GrammarConfig,
		Result<xeno_language::build::BuildStatus, xeno_language::build::GrammarBuildError>,
	)],
) {
	use xeno_language::build::BuildStatus;
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
