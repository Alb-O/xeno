#![cfg_attr(test, allow(unused_crate_dependencies))]
//! Xeno terminal application entry point.

mod cli;
#[cfg(unix)]
mod log_launcher;

use std::ffi::OsStr;

use clap::Parser;
use cli::{Cli, Command, FileLocation, GrammarAction};
use tracing::info;
use xeno_editor::Editor;
use xeno_frontend_tui::run_editor;

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
				xeno_editor::bootstrap_init();
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

	xeno_editor::bootstrap_init();

	let user_config = Editor::load_user_config();

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
	editor.apply_loaded_config(user_config);

	if let Some(theme_name) = cli.theme {
		editor.set_configured_theme_name(theme_name);
	}

	run_editor(editor).await?;
	Ok(())
}

/// Handles grammar fetch/build/sync subcommands.
fn handle_grammar_command(action: GrammarAction) -> anyhow::Result<()> {
	use xeno_language::{build_all_grammars, fetch_all_grammars, load_grammar_configs};

	let configs = load_grammar_configs()?;

	match action {
		GrammarAction::Fetch { only } => {
			let configs: Vec<_> = if let Some(ref names) = only {
				configs.into_iter().filter(|c| names.contains(&c.grammar_id)).collect()
			} else {
				configs
			};
			println!("Fetching {} grammars...", configs.len());
			let results = fetch_all_grammars(configs, None);
			report_fetch_results(&results);
		}
		GrammarAction::Build { only } => {
			let configs: Vec<_> = if let Some(ref names) = only {
				configs.into_iter().filter(|c| names.contains(&c.grammar_id)).collect()
			} else {
				configs
			};
			println!("Building {} grammars...", configs.len());
			let results = build_all_grammars(configs, None);
			report_build_results(&results);
		}
		GrammarAction::Sync { only } => {
			let configs: Vec<_> = if let Some(ref names) = only {
				configs.into_iter().filter(|c| names.contains(&c.grammar_id)).collect()
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
		xeno_language::GrammarConfig,
		Result<xeno_language::FetchStatus, xeno_language::GrammarBuildError>,
	)],
) {
	use xeno_language::FetchStatus;
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
		xeno_language::GrammarConfig,
		Result<xeno_language::BuildStatus, xeno_language::GrammarBuildError>,
	)],
) {
	use xeno_language::BuildStatus;
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

	let _child = log_launcher::spawn_in_terminal(&xeno_path.to_string_lossy(), &args, &socket_path.to_string_lossy())?;

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

	let filter = EnvFilter::try_from_env("XENO_LOG").unwrap_or_else(|_| EnvFilter::new("debug,hyper=info,tower=info"));

	tracing_subscriber::registry().with(filter).with(layer).init();

	info!("Socket tracing initialized");
}

/// Runs the editor with standard initialization for socket logging mode.
async fn run_editor_normal() -> anyhow::Result<()> {
	xeno_editor::bootstrap_init();

	let user_config = Editor::load_user_config();

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
	editor.apply_loaded_config(user_config);

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
		.or_else(xeno_editor::get_data_dir);

	let Some(log_dir) = log_dir else {
		return;
	};

	if std::fs::create_dir_all(&log_dir).is_err() {
		return;
	}

	// Include PID in filename for correlating multi-process logs
	let pid = std::process::id();
	let undo_trace = std::env::var_os("XENO_UNDO_TRACE").is_some();
	let log_path = if undo_trace {
		log_dir.join(format!("xeno.undo-trace.{}.jsonl", pid))
	} else {
		log_dir.join(format!("xeno.{}.log", pid))
	};
	let Ok(file) = OpenOptions::new().create(true).append(true).open(&log_path) else {
		return;
	};

	if undo_trace {
		let filter = EnvFilter::try_from_default_env()
			.or_else(|_| EnvFilter::try_from_env("XENO_LOG"))
			.unwrap_or_else(|_| EnvFilter::new("xeno_undo_trace=trace,warn"));

		let file_layer = tracing_subscriber::fmt::layer()
			.with_writer(file)
			.with_ansi(false)
			.with_span_events(FmtSpan::FULL)
			.with_target(true)
			.json()
			.with_current_span(true)
			.with_span_list(true);

		tracing_subscriber::registry().with(filter).with(file_layer).init();
		info!(path = ?log_path, "Undo tracing initialized");
		return;
	}

	// Support RUST_LOG in addition to XENO_LOG
	let filter = EnvFilter::try_from_default_env()
		.or_else(|_| EnvFilter::try_from_env("XENO_LOG"))
		.unwrap_or_else(|_| EnvFilter::new("xeno_api=debug,xeno_lsp=debug,warn"));

	let file_layer = tracing_subscriber::fmt::layer()
		.with_writer(file)
		.with_ansi(false)
		.with_span_events(FmtSpan::CLOSE)
		.with_target(true);

	tracing_subscriber::registry().with(filter).with(file_layer).init();

	info!(path = ?log_path, "Tracing initialized");
}
