use std::path::PathBuf;

use clap::{Parser, Subcommand};
use xeno_editor::styles::cli_styles;

/// A file path with optional line and column position.
///
/// Supports:
/// - `file.txt` - just a path
/// - `file.txt:42` - path with line number (1-indexed)
/// - `file.txt:42:10` - path with line and column (1-indexed)
/// - `+42 file.txt` - vim-style line number (parsed as two arguments)
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
		let search_start = if s.len() > 2 && s.as_bytes().get(1) == Some(&b':') {
			2
		} else {
			0
		};

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
mod tests {
	use super::*;

	#[test]
	fn parse_plain_path() {
		let loc = FileLocation::parse("foo/bar.txt");
		assert_eq!(loc.path, PathBuf::from("foo/bar.txt"));
		assert_eq!(loc.line, None);
		assert_eq!(loc.column, None);
	}

	#[test]
	fn parse_path_with_line() {
		let loc = FileLocation::parse("foo/bar.txt:42");
		assert_eq!(loc.path, PathBuf::from("foo/bar.txt"));
		assert_eq!(loc.line, Some(41)); // 0-indexed
		assert_eq!(loc.column, None);
	}

	#[test]
	fn parse_path_with_line_and_column() {
		let loc = FileLocation::parse("foo/bar.txt:42:10");
		assert_eq!(loc.path, PathBuf::from("foo/bar.txt"));
		assert_eq!(loc.line, Some(41)); // 0-indexed
		assert_eq!(loc.column, Some(9)); // 0-indexed
	}

	#[test]
	fn parse_line_one_is_zero_indexed() {
		let loc = FileLocation::parse("file.txt:1");
		assert_eq!(loc.line, Some(0));
	}

	#[test]
	fn parse_line_zero_is_treated_as_plain_path() {
		// Line 0 is invalid (1-indexed input), so treat as plain path
		let loc = FileLocation::parse("file.txt:0");
		assert_eq!(loc.path, PathBuf::from("file.txt:0"));
		assert_eq!(loc.line, None);
	}

	#[test]
	fn parse_non_numeric_suffix_is_plain_path() {
		let loc = FileLocation::parse("file.txt:abc");
		assert_eq!(loc.path, PathBuf::from("file.txt:abc"));
		assert_eq!(loc.line, None);
	}

	#[test]
	fn parse_trailing_colon_is_plain_path() {
		let loc = FileLocation::parse("file.txt:");
		assert_eq!(loc.path, PathBuf::from("file.txt:"));
		assert_eq!(loc.line, None);
	}

	#[test]
	fn parse_absolute_path_with_line() {
		let loc = FileLocation::parse("/home/user/file.txt:100");
		assert_eq!(loc.path, PathBuf::from("/home/user/file.txt"));
		assert_eq!(loc.line, Some(99));
	}

	#[test]
	#[cfg(windows)]
	fn parse_windows_path_with_line() {
		let loc = FileLocation::parse("C:\\Users\\test\\file.txt:42");
		assert_eq!(loc.path, PathBuf::from("C:\\Users\\test\\file.txt"));
		assert_eq!(loc.line, Some(41));
	}
}
