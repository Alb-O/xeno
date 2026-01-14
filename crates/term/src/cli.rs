use std::path::PathBuf;

use clap::{Parser, Subcommand};
use xeno_api::styles::cli_styles;

#[derive(Parser, Debug)]
#[command(name = "xeno")]
#[command(about = "A modal text editor")]
#[command(version)]
#[command(styles = cli_styles())]
/// Command-line arguments.
pub struct Cli {
	/// File to edit (opens scratch buffer if omitted)
	pub file: Option<PathBuf>,

	/// Color theme to use (e.g., gruvbox, monokai, debug)
	#[arg(long, short = 't')]
	pub theme: Option<String>,

	/// Launch xeno in a new terminal and show logs in this terminal
	#[arg(long)]
	pub log_launch: bool,

	/// Subcommand to execute.
	#[command(subcommand)]
	pub command: Option<Command>,
}

/// Available subcommands.
#[derive(Subcommand, Debug)]
pub enum Command {
	/// Manage tree-sitter grammars
	Grammar {
		/// Grammar subcommand action.
		#[command(subcommand)]
		action: GrammarAction,
	},
	/// Authentication management
	#[cfg(feature = "auth")]
	Auth {
		/// Auth subcommand action.
		#[command(subcommand)]
		action: AuthAction,
	},
}

/// Authentication subcommands.
#[cfg(feature = "auth")]
#[derive(Subcommand, Debug)]
pub enum AuthAction {
	/// Log in to a service
	Login {
		/// Login provider.
		#[command(subcommand)]
		provider: LoginProvider,
	},
	/// Log out from a service
	Logout {
		/// Logout provider.
		#[command(subcommand)]
		provider: LogoutProvider,
	},
	/// Show authentication status
	Status,
}

/// Login providers.
#[cfg(feature = "auth")]
#[derive(Subcommand, Debug)]
pub enum LoginProvider {
	/// Log in to OpenAI Codex via OAuth
	Codex,
	/// Log in to Anthropic Claude via OAuth
	Claude {
		/// Create API key instead of using OAuth tokens
		#[arg(long)]
		api_key: bool,
	},
}

/// Logout providers.
#[cfg(feature = "auth")]
#[derive(Subcommand, Debug)]
pub enum LogoutProvider {
	/// Log out from OpenAI Codex
	Codex,
	/// Log out from Anthropic Claude
	Claude,
}

/// Grammar management subcommands.
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
