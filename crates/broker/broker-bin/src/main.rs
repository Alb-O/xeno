//! Xeno broker binary.
//!
//! The broker runs as a daemon process and manages:
//! - LSP server processes
//! - AI provider connections
//! - IPC communication with the editor

use std::path::PathBuf;

use clap::Parser;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Broker command line arguments.
#[derive(Parser, Debug)]
#[command(name = "xeno-broker")]
#[command(about = "Xeno language server and AI provider broker")]
struct Args {
	/// Socket path for IPC
	#[arg(short, long, value_name = "PATH")]
	socket: Option<PathBuf>,

	/// Verbose logging
	#[arg(short, long)]
	verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();

	setup_tracing(args.verbose);

	info!("starting xeno-broker");

	let socket_path = args
		.socket
		.unwrap_or_else(xeno_broker_proto::paths::default_socket_path);

	if let Some(parent) = socket_path.parent()
		&& !parent.exists()
	{
		std::fs::create_dir_all(parent)?;
	}

	info!(socket = %socket_path.display(), "IPC socket path");

	let shutdown = CancellationToken::new();

	info!("starting IPC server");
	xeno_broker::ipc::serve(&socket_path, shutdown).await?;

	Ok(())
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
