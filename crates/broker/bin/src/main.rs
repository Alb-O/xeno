//! Xeno broker daemon.
//!
//! The broker coordinates multi-session editing and LSP server lifecycle management.
//! It acts as the central authority for document state and routing.

use std::path::{Path, PathBuf};

use clap::Parser;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Broker command line arguments.
#[derive(Parser, Debug)]
#[command(name = "xeno-broker")]
#[command(about = "Xeno language server and AI provider broker")]
struct Args {
	/// Path to the Unix domain socket for IPC.
	#[arg(short, long, value_name = "PATH")]
	socket: Option<PathBuf>,

	/// Enable verbose debug logging.
	#[arg(short, long)]
	verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();
	setup_tracing(args.verbose);

	info!("Starting xeno-broker");

	let socket_path = args
		.socket
		.unwrap_or_else(xeno_broker_proto::paths::default_socket_path);

	ensure_parent_dir(&socket_path)?;

	let shutdown = CancellationToken::new();
	spawn_signal_handler(shutdown.clone());

	info!(socket = %socket_path.display(), "Starting IPC server");
	xeno_broker::ipc::serve(&socket_path, shutdown.clone()).await?;

	cleanup_socket(&socket_path);
	Ok(())
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent)?;
	}
	Ok(())
}

fn spawn_signal_handler(shutdown: CancellationToken) {
	tokio::spawn(async move {
		use tokio::signal::unix::{SignalKind, signal};
		let mut sigterm =
			signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
		let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");

		tokio::select! {
			_ = sigterm.recv() => info!("Received SIGTERM, shutting down"),
			_ = sigint.recv() => info!("Received SIGINT, shutting down"),
		}
		shutdown.cancel();
	});
}

fn cleanup_socket(path: &Path) {
	if path.exists() {
		let _ = std::fs::remove_file(path);
	}
}

fn setup_tracing(verbose: bool) {
	use std::fs::OpenOptions;

	use tracing_subscriber::EnvFilter;
	use tracing_subscriber::fmt::format::FmtSpan;
	use tracing_subscriber::prelude::*;

	// Support XENO_LOG_DIR for smoke testing
	if let Some(log_dir) = std::env::var("XENO_LOG_DIR").ok().map(PathBuf::from)
		&& std::fs::create_dir_all(&log_dir).is_ok()
	{
		let pid = std::process::id();
		let log_path = log_dir.join(format!("xeno-broker.{}.log", pid));

		if let Ok(file) = OpenOptions::new().create(true).append(true).open(&log_path) {
			let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
				if verbose {
					EnvFilter::new("xeno_broker=trace,debug")
				} else {
					EnvFilter::new("xeno_broker=debug,info")
				}
			});

			let file_layer = tracing_subscriber::fmt::layer()
				.with_writer(file)
				.with_ansi(false)
				.with_span_events(FmtSpan::CLOSE)
				.with_target(true);

			tracing_subscriber::registry()
				.with(filter)
				.with(file_layer)
				.init();

			tracing::info!(path = ?log_path, "Broker tracing initialized");
			return;
		}
	}

	// Fallback to stderr-only logging
	tracing_subscriber::fmt()
		.with_max_level(if verbose {
			tracing::Level::DEBUG
		} else {
			tracing::Level::INFO
		})
		.init();
}
