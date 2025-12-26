use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tome_api::styles::cli_styles;

#[derive(Parser, Debug)]
#[command(name = "tome")]
#[command(about = "A modal text editor")]
#[command(version)]
#[command(styles = cli_styles())]
pub struct Cli {
	/// File to edit (opens scratch buffer if omitted)
	pub file: Option<PathBuf>,

	#[command(subcommand)]
	pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
	/// Manage tree-sitter grammars
	Grammar {
		#[command(subcommand)]
		action: GrammarAction,
	},
}

#[derive(Subcommand, Debug)]
pub enum GrammarAction {
	/// Fetch grammar sources from git repositories
	Fetch {
		/// Only fetch specific grammars (comma-separated)
		#[arg(long, value_delimiter = ',')]
		only: Option<Vec<String>>,
	},
	/// Build grammar shared libraries
	Build {
		/// Only build specific grammars (comma-separated)
		#[arg(long, value_delimiter = ',')]
		only: Option<Vec<String>>,
	},
	/// Fetch and build all grammars
	Sync {
		/// Only sync specific grammars (comma-separated)
		#[arg(long, value_delimiter = ',')]
		only: Option<Vec<String>>,
	},
}
