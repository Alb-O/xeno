#![cfg_attr(test, allow(unused_crate_dependencies))]
//! Xeno terminal application entry point.

mod app;
mod backend;
mod cli;
#[cfg(unix)]
mod log_launcher;
mod terminal;

use std::ffi::OsStr;

use app::run_editor;
use clap::Parser;
use cli::{Cli, Command, FileLocation, GrammarAction};
use tracing::{info, warn};
use xeno_editor::Editor;
use xeno_registry::options::keys;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	#[cfg(unix)]
	if let Ok(socket_path) = std::env::var(log_launcher::LOG_SINK_ENV) {
		return run_with_socket_logging(&socket_path).await;
	}

	let cli = Cli::parse();

	#[cfg(unix)]
	if cli.log_launch {
		return run_log_launcher_mode(&cli);
	}

	setup_tracing();

	match cli.command {
		Some(Command::Grammar { action }) => return handle_grammar_command(action),
		Some(Command::LspSmoke { workspace }) => {
			#[cfg(feature = "lsp")]
			{
				xeno_editor::bootstrap::init();
				return xeno_editor::run_lsp_smoke(workspace).await;
			}
			#[cfg(not(feature = "lsp"))]
			{
				let _ = workspace;
				anyhow::bail!("LSP support is not enabled in this build");
			}
		}
		None => {}
	}

	xeno_editor::bootstrap::init();

	let user_config = load_user_config();

	let mut editor = match cli.file_location() {
		Some(loc) => {
			let mut ed = Editor::new_with_path(loc.path);
			if let Some(line) = loc.line {
				ed.set_deferred_goto(line, loc.column.unwrap_or(0));
			}
			ed
		}
		None => Editor::new_scratch(),
	};

	editor.kick_theme_load();
	editor.kick_lsp_catalog_load();
	apply_user_config(&mut editor, user_config);

	if let Some(theme_name) = cli.theme {
		use xeno_registry::options::OptionValue;
		let opt = xeno_registry::db::OPTIONS
			.get_key(&keys::THEME.untyped())
			.expect("theme option missing from registry");
		editor
			.config_mut()
			.global_options
			.set(opt, OptionValue::String(theme_name));
	}

	run_editor(editor).await?;
	Ok(())
}

/// Loads user config from `~/.config/xeno/config.kdl`.
fn load_user_config() -> Option<xeno_registry::config::Config> {
	use xeno_registry::config::kdl::parse_config_str;

	let config_path = xeno_editor::paths::get_config_dir()?.join("config.kdl");
	if !config_path.exists() {
		return None;
	}

	let content = match std::fs::read_to_string(&config_path) {
		Ok(c) => c,
		Err(e) => {
			warn!(path = %config_path.display(), error = %e, "failed to read config file");
			return None;
		}
	};

	match parse_config_str(&content) {
		Ok(config) => {
			for warning in &config.warnings {
				warn!("{warning}");
			}
			Some(config)
		}
		Err(e) => {
			warn!(error = %e, "failed to parse config");
			None
		}
	}
}

/// Applies user config options to the editor.
///
/// Theme preferences are stored but not resolved until themes finish loading.
fn apply_user_config(editor: &mut Editor, config: Option<xeno_registry::config::Config>) {
	let Some(config) = config else { return };

	editor.config_mut().global_options.merge(&config.options);

	for lang_config in config.languages {
		editor
			.config_mut()
			.language_options
			.entry(lang_config.name)
			.or_default()
			.merge(&lang_config.options);
	}
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
#[cfg(unix)]
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
#[cfg(unix)]
async fn run_with_socket_logging(socket_path: &str) -> anyhow::Result<()> {
	setup_socket_tracing(socket_path);
	run_editor_normal().await
}

/// Configures tracing to send events over a Unix socket to the log viewer.
#[cfg(unix)]
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

/// Runs the editor with standard initialization for socket logging mode.
async fn run_editor_normal() -> anyhow::Result<()> {
	xeno_editor::bootstrap::init();

	let user_config = load_user_config();

	let mut editor = match std::env::args().nth(1) {
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

	editor.kick_theme_load();
	editor.kick_lsp_catalog_load();
	apply_user_config(&mut editor, user_config);

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

	// Support XENO_LOG_DIR for smoke testing, fall back to data dir
	let log_dir = std::env::var("XENO_LOG_DIR")
		.ok()
		.map(std::path::PathBuf::from)
		.or_else(xeno_editor::paths::get_data_dir);

	let Some(log_dir) = log_dir else {
		return;
	};

	if std::fs::create_dir_all(&log_dir).is_err() {
		return;
	}

	// Include PID in filename for correlating multi-process logs
	let pid = std::process::id();
	let log_path = log_dir.join(format!("xeno.{}.log", pid));
	let Ok(file) = OpenOptions::new().create(true).append(true).open(&log_path) else {
		return;
	};

	// Support RUST_LOG in addition to XENO_LOG
	let filter = EnvFilter::try_from_default_env()
		.or_else(|_| EnvFilter::try_from_env("XENO_LOG"))
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
