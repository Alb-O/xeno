use std::path::PathBuf;

use clap::Parser;
use tome_api::styles::cli_styles;

#[derive(Parser, Debug)]
#[command(name = "tome")]
#[command(about = "A modal text editor")]
#[command(version)]
#[command(styles = cli_styles())]
pub struct Cli {
	/// File to edit (opens scratch buffer if omitted)
	pub file: Option<PathBuf>,

	/// Execute an Ex command at startup (e.g. "acp.start")
	#[arg(long = "ex", short = 'c')]
	pub ex: Option<String>,

	/// Exit immediately after running `--ex`
	#[arg(long, short = 'q')]
	pub quit_after_ex: bool,
}
