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

	tracing_subscriber::fmt()
		.with_max_level(if args.verbose {
			tracing::Level::DEBUG
		} else {
			tracing::Level::INFO
		})
		.init();

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

	let core = xeno_broker::core::BrokerCore::new();
	let shutdown = CancellationToken::new();

	info!("starting IPC server");
	xeno_broker::ipc::serve(&socket_path, core, shutdown).await?;

	Ok(())
}
