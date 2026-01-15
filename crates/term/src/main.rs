//! Xeno terminal application entry point.

mod app;
mod backend;
mod cli;
mod log_launcher;
mod terminal;
#[cfg(test)]
mod tests;

use std::ffi::OsStr;
use std::path::PathBuf;

use app::run_editor;
use clap::Parser;
#[cfg(feature = "auth")]
use cli::{AuthAction, LoginProvider, LogoutProvider};
use cli::{Cli, Command, GrammarAction};
use tracing::{info, warn};
use xeno_api::Editor;
// Force-link crates to ensure their distributed_slice registrations are included.
#[allow(unused_imports, reason = "linkme distributed_slice registration")]
use xeno_core as _;
use xeno_registry::options::keys;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	if let Ok(socket_path) = std::env::var(log_launcher::LOG_SINK_ENV) {
		return run_with_socket_logging(&socket_path).await;
	}

	let cli = Cli::parse();

	if cli.log_launch {
		return run_log_launcher_mode(&cli);
	}

	setup_tracing();

	match cli.command {
		Some(Command::Grammar { action }) => return handle_grammar_command(action),
		#[cfg(feature = "auth")]
		Some(Command::Auth { action }) => return handle_auth_command(action).await,
		None => {}
	}

	// Ensure runtime directory is populated with query files
	if let Err(e) = xeno_runtime_language::ensure_runtime() {
		warn!(error = %e, "failed to seed runtime");
	}

	// Load themes from runtime directory
	let themes_dir = xeno_runtime_language::runtime_dir().join("themes");
	let mut theme_errors = Vec::new();
	match xeno_runtime_config::load_and_register_themes(&themes_dir) {
		Ok(errors) => theme_errors.extend(errors),
		Err(e) => warn!(error = %e, "failed to read themes directory"),
	}

	// Load user config if present
	let user_config = if let Some(config_dir) = xeno_api::paths::get_config_dir() {
		let config_path = config_dir.join("config.kdl");
		if config_path.exists() {
			match xeno_runtime_config::Config::load(&config_path) {
				Ok(config) => {
					for warning in &config.warnings {
						warn!("{warning}");
					}
					Some(config)
				}
				Err(e) => {
					warn!(error = %e, "failed to load config");
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

	configure_lsp_servers(&mut editor);

	// Initialize LSP for the initial buffer (opened before servers were configured)
	if let Err(e) = editor.init_lsp_for_open_buffers().await {
		warn!(error = %e, "Failed to initialize LSP for initial buffer");
	}

	// Apply user config to editor
	if let Some(config) = user_config {
		// Apply global options
		editor.config.global_options.merge(&config.options);

		// Apply language-specific options
		for lang_config in config.languages {
			editor
				.config
				.language_options
				.entry(lang_config.name)
				.or_default()
				.merge(&lang_config.options);
		}

		// Apply theme from config if specified
		if let Some(theme_name) = config.options.get_string(keys::THEME.untyped())
			&& let Err(e) = editor.set_theme(theme_name)
		{
			warn!(theme = theme_name, error = %e, "failed to set config theme");
		}
	}

	// CLI theme flag overrides config
	if let Some(theme_name) = cli.theme
		&& let Err(e) = editor.set_theme(&theme_name)
	{
		warn!(theme = %theme_name, error = %e, "failed to set theme");
	}

	for (filename, error) in theme_errors {
		editor.notify(xeno_registry::notification_keys::error::call(format!(
			"{filename}: {error}"
		)));
	}

	run_editor(editor).await?;
	Ok(())
}

/// Handles auth login/logout/status subcommands.
#[cfg(feature = "auth")]
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
	use xeno_runtime_language::build::{
		build_all_grammars, fetch_all_grammars, load_grammar_configs,
	};

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
		xeno_runtime_language::build::GrammarConfig,
		Result<
			xeno_runtime_language::build::FetchStatus,
			xeno_runtime_language::build::GrammarBuildError,
		>,
	)],
) {
	use xeno_runtime_language::build::FetchStatus;
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
				println!("  ✗ {name}: {e}");
				failed += 1;
			}
		}
	}

	println!("\nFetch: {success} succeeded, {skipped} skipped, {failed} failed");
}

/// Prints a summary of grammar build results to stdout.
fn report_build_results(
	results: &[(
		xeno_runtime_language::build::GrammarConfig,
		Result<
			xeno_runtime_language::build::BuildStatus,
			xeno_runtime_language::build::GrammarBuildError,
		>,
	)],
) {
	use xeno_runtime_language::build::BuildStatus;
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
				println!("  ✗ {name}: {e}");
				failed += 1;
			}
		}
	}

	println!("\nBuild: {success} succeeded, {skipped} skipped, {failed} failed");
}

/// Spawns xeno in a new terminal window and runs the log viewer in this terminal.
fn run_log_launcher_mode(cli: &Cli) -> anyhow::Result<()> {
	let socket_path = std::env::temp_dir().join(format!("xeno-log-{}.sock", uuid::Uuid::new_v4()));
	let xeno_path = std::env::current_exe()?;

	let mut args: Vec<&OsStr> = Vec::new();
	if let Some(ref file) = cli.file {
		args.push(file.as_os_str());
	}
	if let Some(ref theme) = cli.theme {
		args.push(OsStr::new("--theme"));
		args.push(OsStr::new(theme));
	}

	let _child = log_launcher::spawn_in_terminal(
		&xeno_path.to_string_lossy(),
		&args,
		&socket_path.to_string_lossy(),
	)?;

	log_launcher::run_log_viewer(&socket_path)?;
	Ok(())
}

/// Runs xeno with socket-based logging (child process spawned by `--log-launch`).
async fn run_with_socket_logging(socket_path: &str) -> anyhow::Result<()> {
	setup_socket_tracing(socket_path);
	run_editor_normal().await
}

/// Configures tracing to send events over a Unix socket to the log viewer.
fn setup_socket_tracing(socket_path: &str) {
	use tracing_subscriber::EnvFilter;
	use tracing_subscriber::prelude::*;

	let Ok(layer) = log_launcher::SocketLayer::new(socket_path) else {
		setup_tracing();
		return;
	};

	let filter = EnvFilter::try_from_env("XENO_LOG")
		.unwrap_or_else(|_| EnvFilter::new("debug,hyper=info,tower=info"));

	tracing_subscriber::registry()
		.with(filter)
		.with(layer)
		.init();

	info!("Socket tracing initialized");
}

/// Runs the editor with standard initialization (used by socket logging mode).
async fn run_editor_normal() -> anyhow::Result<()> {
	if let Err(e) = xeno_runtime_language::ensure_runtime() {
		warn!(error = %e, "failed to seed runtime");
	}

	let themes_dir = xeno_runtime_language::runtime_dir().join("themes");
	let theme_errors = match xeno_runtime_config::load_and_register_themes(&themes_dir) {
		Ok(errors) => errors,
		Err(e) => {
			warn!(error = %e, "failed to read themes directory");
			Vec::new()
		}
	};

	let user_config = xeno_api::paths::get_config_dir()
		.map(|d| d.join("config.kdl"))
		.filter(|p| p.exists())
		.and_then(
			|config_path| match xeno_runtime_config::Config::load(&config_path) {
				Ok(config) => {
					for warning in &config.warnings {
						warn!("{warning}");
					}
					Some(config)
				}
				Err(e) => {
					warn!(error = %e, "failed to load config");
					None
				}
			},
		);

	let file: Option<PathBuf> = std::env::args().nth(1).map(PathBuf::from);
	let mut editor = match file {
		Some(path) if path.exists() || !path.to_string_lossy().starts_with('-') => {
			Editor::new(path).await?
		}
		_ => Editor::new_scratch(),
	};

	configure_lsp_servers(&mut editor);

	if let Err(e) = editor.init_lsp_for_open_buffers().await {
		warn!(error = %e, "Failed to initialize LSP for initial buffer");
	}

	if let Some(config) = user_config {
		editor.config.global_options.merge(&config.options);
		for lang_config in config.languages {
			editor
				.config
				.language_options
				.entry(lang_config.name)
				.or_default()
				.merge(&lang_config.options);
		}
		if let Some(theme_name) = config.options.get_string(keys::THEME.untyped())
			&& let Err(e) = editor.set_theme(theme_name)
		{
			warn!(theme = theme_name, error = %e, "failed to set config theme");
		}
	}

	for (filename, error) in theme_errors {
		editor.notify(xeno_registry::notification_keys::error::call(format!(
			"{filename}: {error}"
		)));
	}

	run_editor(editor).await?;
	Ok(())
}

/// Sets up tracing to log to a file in the data directory.
///
/// Logs go to `~/.local/share/xeno/xeno.log` (or platform equivalent).
/// Set `XENO_LOG` env var to control filtering (e.g., `XENO_LOG=debug` or `XENO_LOG=xeno_lsp=trace`).
fn setup_tracing() {
	use std::fs::OpenOptions;

	use tracing_subscriber::EnvFilter;
	use tracing_subscriber::fmt::format::FmtSpan;
	use tracing_subscriber::prelude::*;

	let Some(data_dir) = xeno_api::paths::get_data_dir() else {
		return;
	};

	if std::fs::create_dir_all(&data_dir).is_err() {
		return;
	}

	let log_path = data_dir.join("xeno.log");
	let Ok(file) = OpenOptions::new().create(true).append(true).open(&log_path) else {
		return;
	};

	let filter = EnvFilter::try_from_env("XENO_LOG")
		.unwrap_or_else(|_| EnvFilter::new("xeno_api=debug,xeno_lsp=debug,warn"));

	let file_layer = tracing_subscriber::fmt::layer()
		.with_writer(file)
		.with_ansi(false)
		.with_span_events(FmtSpan::CLOSE)
		.with_target(true);

	tracing_subscriber::registry()
		.with(filter)
		.with(file_layer)
		.init();

	info!(path = ?log_path, "Tracing initialized");
}

/// Configures language servers from embedded `lsp.kdl` and `languages.kdl`.
fn configure_lsp_servers(editor: &mut Editor) {
	let Ok(server_defs) = xeno_runtime_language::load_lsp_configs() else {
		return;
	};
	let Ok(lang_mapping) = xeno_runtime_language::load_language_lsp_mapping() else {
		return;
	};

	let server_map: std::collections::HashMap<_, _> =
		server_defs.iter().map(|s| (s.name.as_str(), s)).collect();

	for (language, info) in &lang_mapping {
		// Try each configured server in order until one with an available binary is found
		let Some(server_def) = info.servers.iter().find_map(|name| {
			let def = server_map.get(name.as_str())?;
			which::which(&def.command).ok().map(|_| def)
		}) else {
			continue;
		};
		editor.lsp.configure_server(
			language.clone(),
			xeno_api::lsp::LanguageServerConfig {
				command: server_def.command.clone(),
				args: server_def.args.clone(),
				env: server_def.environment.clone(),
				root_markers: info.roots.clone(),
				config: server_def.config.clone(),
				..Default::default()
			},
		);
	}
}
