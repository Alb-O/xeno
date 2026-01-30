//! Xeno broker binary.
//!
//! The broker runs as a daemon process and manages:
//! - LSP server processes
//! - AI provider connections
//! - IPC communication with the editor

use std::path::PathBuf;

use clap::Parser;
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

	// Initialize tracing
	let subscriber = tracing_subscriber::fmt()
		.with_max_level(if args.verbose {
			tracing::Level::DEBUG
		} else {
			tracing::Level::INFO
		})
		.finish();

	tracing::subscriber::set_global_default(subscriber)?;

	info!("Starting xeno-broker");

	// Determine socket path
	let socket_path = args.socket.unwrap_or_else(|| {
		let runtime_dir = dirs::runtime_dir()
			.or_else(dirs::cache_dir)
			.unwrap_or_else(std::env::temp_dir);
		runtime_dir.join("xeno-broker.sock")
	});

	info!(socket = %socket_path.display(), "IPC socket path");

	// Initialize broker core
	let core = xeno_broker::core::BrokerCore::new();

	// Start IPC server
	info!("Starting IPC server");
	xeno_broker::ipc::serve(&socket_path, core).await?;

	Ok(())
}
