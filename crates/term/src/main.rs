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
use cli::{AuthAction, Cli, Command, GrammarAction, LoginProvider, LogoutProvider};
use xeno_api::Editor;
use xeno_registry::options::keys;
// Force-link crates to ensure their distributed_slice registrations are included.
#[allow(unused_imports, reason = "linkme distributed_slice registration")]
use {xeno_acp as _, xeno_core as _, xeno_extensions as _};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	// Handle subcommands before starting the editor
	match cli.command {
		Some(Command::Grammar { action }) => return handle_grammar_command(action),
		Some(Command::Auth { action }) => return handle_auth_command(action).await,
		None => {}
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

	// Load user config if present
	let user_config = if let Some(config_dir) = xeno_api::paths::get_config_dir() {
		let config_path = config_dir.join("config.kdl");
		if config_path.exists() {
			match xeno_config::Config::load(&config_path) {
				Ok(config) => {
					// Display any config warnings
					for warning in &config.warnings {
						eprintln!("Warning: {warning}");
					}
					Some(config)
				}
				Err(e) => {
					eprintln!("Warning: failed to load config: {}", e);
					None
				}
			}
		} else {
			None
		}
	} else {
		None
	};

	let mut editor = match cli.file {
		Some(path) => Editor::new(path).await?,
		None => Editor::new_scratch(),
	};

	// Apply user config to editor
	if let Some(config) = user_config {
		// Apply global options
		editor.global_options.merge(&config.options);

		// Apply language-specific options
		for lang_config in config.languages {
			editor
				.language_options
				.entry(lang_config.name)
				.or_default()
				.merge(&lang_config.options);
		}

		// Apply theme from config if specified
		if let Some(theme_name) = config.options.get_string(keys::THEME.untyped())
			&& let Err(e) = editor.set_theme(theme_name)
		{
			eprintln!("Warning: failed to set config theme '{}': {}", theme_name, e);
		}
	}

	// CLI theme flag overrides config
	if let Some(theme_name) = cli.theme
		&& let Err(e) = editor.set_theme(&theme_name)
	{
		eprintln!("Warning: failed to set theme '{}': {}", theme_name, e);
	}

	run_editor(editor).await?;
	Ok(())
}

/// Handles auth login/logout/status subcommands.
async fn handle_auth_command(action: AuthAction) -> anyhow::Result<()> {
	use xeno_auth::default_data_dir;

	let data_dir = default_data_dir()?;

	match action {
		AuthAction::Login { provider } => match provider {
			LoginProvider::Codex => {
				use xeno_auth::codex::{LoginConfig, start_login};
				let config = LoginConfig::new(data_dir);
				let server = start_login(config)?;
				println!("Opening browser for authentication...");
				println!("If browser doesn't open, visit: {}", server.auth_url);
				server.wait().await?;
				println!("Login successful!");
			}
			LoginProvider::Claude { api_key } => {
				use xeno_auth::claude::{LoginMode, complete_login, start_login};
				let mode = if api_key {
					LoginMode::Console
				} else {
					LoginMode::Max
				};
				let session = start_login(data_dir, mode);
				println!("Open this URL in your browser:");
				println!("  {}", session.auth_url);
				println!();
				println!("After authentication, paste the authorization code below.");
				print!("Code: ");
				use std::io::Write;
				std::io::stdout().flush()?;
				let mut code = String::new();
				std::io::stdin().read_line(&mut code)?;
				complete_login(&session, code.trim()).await?;
				println!("Login successful!");
			}
		},
		AuthAction::Logout { provider } => match provider {
			LogoutProvider::Codex => {
				use xeno_auth::codex::logout;
				if logout(&data_dir)? {
					println!("Logged out from Codex.");
				} else {
					println!("Not logged in to Codex.");
				}
			}
			LogoutProvider::Claude => {
				use xeno_auth::claude::logout;
				if logout(&data_dir)? {
					println!("Logged out from Claude.");
				} else {
					println!("Not logged in to Claude.");
				}
			}
		},
		AuthAction::Status => {
			use xeno_auth::claude::load_auth as load_claude;
			use xeno_auth::codex::load_auth as load_codex;

			match load_codex(&data_dir)? {
				Some(auth) if auth.api_key.is_some() => {
					println!("Codex: authenticated (API key)");
				}
				Some(auth) if auth.tokens.is_some() => {
					let email = auth
						.tokens
						.as_ref()
						.and_then(|t| t.id_token.email.as_deref())
						.unwrap_or("<unknown>");
					println!("Codex: authenticated as {email}");
				}
				_ => {
					println!("Codex: not authenticated");
				}
			}

			match load_claude(&data_dir)? {
				Some(auth) if auth.api_key.is_some() => {
					println!("Claude: authenticated (API key)");
				}
				Some(auth) if auth.oauth.is_some() => {
					println!("Claude: authenticated (OAuth)");
				}
				_ => {
					println!("Claude: not authenticated");
				}
			}
		}
	}

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
