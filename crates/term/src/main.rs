//! Xeno terminal application entry point.

mod app;
mod backend;
mod cli;
mod log_launcher;
mod splash;
mod terminal;
#[cfg(test)]
mod tests;

use std::ffi::OsStr;

use app::run_editor;
use clap::Parser;
use cli::{Cli, Command, FileLocation, GrammarAction};
use tracing::{info, warn};
use xeno_editor::Editor;
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

	let startup_time = std::time::Instant::now();
	let log_buffer = splash::new_log_buffer();
	setup_tracing(Some(log_buffer.clone()));

	match cli.command {
		Some(Command::Grammar { action }) => return handle_grammar_command(action),
		None => {}
	}

	let runtime_status = match xeno_runtime_language::ensure_runtime() {
		Ok(status) => Some(status),
		Err(e) => {
			warn!(error = %e, "failed to seed runtime");
			None
		}
	};

	// Load themes from runtime directory
	let themes_dir = xeno_runtime_language::runtime_dir().join("themes");
	let mut theme_errors = Vec::new();
	match xeno_runtime_config::load_and_register_themes(&themes_dir) {
		Ok(errors) => theme_errors.extend(errors),
		Err(e) => warn!(error = %e, "failed to read themes directory"),
	}

	// Load user config if present
	let user_config = if let Some(config_dir) = xeno_editor::paths::get_config_dir() {
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

	let mut editor = match cli.file_location() {
		Some(loc) => {
			let mut ed = Editor::new(loc.path).await?;
			if let Some(line) = loc.line {
				ed.goto_line_col(line, loc.column.unwrap_or(0));
			}
			ed
		}
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
		editor.notify(xeno_registry::notification_keys::error(format!(
			"{filename}: {error}"
		)));
	}

	if let Some(xeno_runtime_language::RuntimeStatus::Outdated { local, expected }) = runtime_status
	{
		editor.notify(xeno_registry::notification_keys::warn(format!(
			"Runtime assets from v{local} (current: v{expected}). Run :reseed to update."
		)));
	}

	run_editor(editor, Some((log_buffer, startup_time))).await?;
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
		args.push(OsStr::new(file));
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
		setup_tracing(None);
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
	let runtime_status = match xeno_runtime_language::ensure_runtime() {
		Ok(status) => Some(status),
		Err(e) => {
			warn!(error = %e, "failed to seed runtime");
			None
		}
	};

	let themes_dir = xeno_runtime_language::runtime_dir().join("themes");
	let theme_errors = match xeno_runtime_config::load_and_register_themes(&themes_dir) {
		Ok(errors) => errors,
		Err(e) => {
			warn!(error = %e, "failed to read themes directory");
			Vec::new()
		}
	};

	let user_config = xeno_editor::paths::get_config_dir()
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

	let file_arg = std::env::args().nth(1);
	let mut editor = match file_arg {
		Some(arg) if !arg.starts_with('-') => {
			let loc = FileLocation::parse(&arg);
			let mut ed = Editor::new(loc.path).await?;
			if let Some(line) = loc.line {
				ed.goto_line_col(line, loc.column.unwrap_or(0));
			}
			ed
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
		editor.notify(xeno_registry::notification_keys::error(format!(
			"{filename}: {error}"
		)));
	}

	if let Some(xeno_runtime_language::RuntimeStatus::Outdated { local, expected }) = runtime_status
	{
		editor.notify(xeno_registry::notification_keys::warn(format!(
			"Runtime assets from v{local} (current: v{expected}). Run :reseed to update."
		)));
	}

	run_editor(editor, None).await?;
	Ok(())
}

/// Sets up tracing to log to a file in the data directory.
///
/// Logs go to `~/.local/share/xeno/xeno.log` (or platform equivalent).
/// Set `XENO_LOG` env var to control filtering (e.g., `XENO_LOG=debug` or `XENO_LOG=xeno_lsp=trace`).
///
/// If a log buffer is provided, also registers a splash screen layer to capture
/// recent logs for display during startup.
fn setup_tracing(log_buffer: Option<splash::LogBuffer>) {
	use std::fs::OpenOptions;

	use tracing_subscriber::EnvFilter;
	use tracing_subscriber::fmt::format::FmtSpan;
	use tracing_subscriber::prelude::*;

	let Some(data_dir) = xeno_editor::paths::get_data_dir() else {
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

	let splash_layer = log_buffer.map(splash::SplashLogLayer::new);

	tracing_subscriber::registry()
		.with(filter)
		.with(file_layer)
		.with(splash_layer)
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
			xeno_editor::lsp::LanguageServerConfig {
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
