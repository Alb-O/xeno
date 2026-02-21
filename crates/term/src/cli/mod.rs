//! CLI schema and parsing helpers for the xeno binary.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use xeno_editor::cli_styles;

/// A file path with optional line and column position.
///
/// Supports:
/// * `file.txt` - just a path
/// * `file.txt:42` - path with line number (1-indexed)
/// * `file.txt:42:10` - path with line and column (1-indexed)
/// * `+42 file.txt` - vim-style line number (parsed as two arguments)
#[derive(Debug, Clone, Default)]
pub struct FileLocation {
	/// The file path.
	pub path: PathBuf,
	/// Line number (0-indexed). None means start of file.
	pub line: Option<usize>,
	/// Column number (0-indexed). None means start of line.
	pub column: Option<usize>,
}

impl FileLocation {
	/// Parses a file location from a string.
	///
	/// Handles `path:line` and `path:line:col` formats.
	/// Line and column in input are 1-indexed, converted to 0-indexed.
	pub fn parse(s: &str) -> Self {
		Self::parse_colon_format(s).unwrap_or_else(|| Self {
			path: PathBuf::from(s),
			line: None,
			column: None,
		})
	}

	/// Parses `path:line` or `path:line:col` format.
	///
	/// Skips the first 2 chars when searching for colons to handle Windows
	/// drive letters (e.g., `C:\foo\bar.txt:42`).
	fn parse_colon_format(s: &str) -> Option<Self> {
		let search_start = if s.len() > 2 && s.as_bytes().get(1) == Some(&b':') { 2 } else { 0 };

		let suffix = &s[search_start..];
		let last_colon = suffix.rfind(':')?;
		let last_colon_abs = search_start + last_colon;
		let after_last = &s[last_colon_abs + 1..];

		if after_last.is_empty() {
			return None;
		}

		// Try path:line:col first
		if let Some(second_last_colon) = suffix[..last_colon].rfind(':') {
			let second_last_abs = search_start + second_last_colon;
			let line_str = &s[second_last_abs + 1..last_colon_abs];

			if let (Ok(line), Ok(col)) = (line_str.parse::<usize>(), after_last.parse::<usize>())
				&& line > 0
			{
				return Some(Self {
					path: PathBuf::from(&s[..second_last_abs]),
					line: Some(line - 1),
					column: Some(col.saturating_sub(1)),
				});
			}
		}

		// Try path:line
		if let Ok(line) = after_last.parse::<usize>()
			&& line > 0
		{
			return Some(Self {
				path: PathBuf::from(&s[..last_colon_abs]),
				line: Some(line - 1),
				column: None,
			});
		}

		None
	}
}

#[derive(Parser, Debug)]
#[command(name = "xeno")]
#[command(about = "A modal text editor")]
#[command(version)]
#[command(styles = cli_styles())]
/// Command-line arguments.
pub struct Cli {
	/// Line number to jump to (vim-style +N)
	#[arg(short = 'l', long = "line", value_name = "LINE")]
	pub goto_line: Option<usize>,

	/// File to edit (opens scratch buffer if omitted).
	/// Supports path:line and path:line:col formats.
	pub file: Option<String>,

	/// Color theme to use (e.g., gruvbox, monokai, debug)
	#[arg(long, short = 't')]
	pub theme: Option<String>,

	/// Launch xeno in a new terminal and show logs in this terminal (Unix only)
	#[cfg(unix)]
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
	/// Run headless LSP smoke test
	LspSmoke {
		/// Path to workspace directory with Cargo.toml (defaults to current dir)
		workspace: Option<PathBuf>,
	},
}

impl Cli {
	/// Returns the parsed file location from CLI arguments.
	///
	/// Combines the file path with any `-l`/`--line` argument.
	/// The `-l` flag takes precedence over `:line` suffix in the path.
	pub fn file_location(&self) -> Option<FileLocation> {
		let mut loc = FileLocation::parse(self.file.as_ref()?);
		if let Some(line) = self.goto_line {
			loc.line = Some(line.saturating_sub(1));
		}
		Some(loc)
	}
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

#[cfg(test)]
mod tests;
