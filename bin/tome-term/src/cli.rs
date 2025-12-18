use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::styles::cli_styles;

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

	#[command(subcommand)]
	pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
	/// Plugin management
	Plugin(PluginArgs),
}

#[derive(Args, Debug)]
pub struct PluginArgs {
	#[command(subcommand)]
	pub command: PluginCommands,
}

#[derive(Subcommand, Debug)]
pub enum PluginCommands {
	/// Add a plugin from a local path
	Add {
		/// Path to the plugin
		path: PathBuf,
		/// Register as a development plugin
		#[arg(long)]
		dev: bool,
	},
	/// List installed plugins
	List,
	/// Remove plugins
	Remove {
		/// IDs of the plugins to remove
		ids: Vec<String>,
	},
	/// Enable plugins
	Enable {
		/// IDs of the plugins to enable
		ids: Vec<String>,
	},
	/// Disable plugins
	Disable {
		/// IDs of the plugins to disable
		ids: Vec<String>,
	},
	/// Reload plugins (experimental)
	Reload {
		/// IDs of the plugins to reload
		ids: Vec<String>,
	},
}
