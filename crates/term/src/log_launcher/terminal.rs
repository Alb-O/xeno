//! Terminal detection and spawning for log launcher mode.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

/// Known terminal emulators in order of preference.
const KNOWN_TERMINALS: &[TerminalKind] = &[
	TerminalKind::Kitty,
	TerminalKind::Alacritty,
	TerminalKind::WezTerm,
	TerminalKind::Foot,
	TerminalKind::GnomeTerminal,
	TerminalKind::Konsole,
	TerminalKind::Xterm,
];

/// Supported terminal emulators with their spawn conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalKind {
	Kitty,
	Alacritty,
	WezTerm,
	Foot,
	GnomeTerminal,
	Konsole,
	Xterm,
	/// Generic terminal using -e flag convention.
	Generic,
}

impl TerminalKind {
	/// Returns the executable name for this terminal.
	pub fn executable(&self) -> &'static str {
		match self {
			TerminalKind::Kitty => "kitty",
			TerminalKind::Alacritty => "alacritty",
			TerminalKind::WezTerm => "wezterm",
			TerminalKind::Foot => "foot",
			TerminalKind::GnomeTerminal => "gnome-terminal",
			TerminalKind::Konsole => "konsole",
			TerminalKind::Xterm => "xterm",
			TerminalKind::Generic => "x-terminal-emulator",
		}
	}

	/// Builds a command to spawn this terminal with the given program and args.
	pub fn build_command(&self, program: &str, args: &[&OsStr]) -> Command {
		let mut cmd = Command::new(self.executable());
		match self {
			TerminalKind::Kitty
			| TerminalKind::Alacritty
			| TerminalKind::Konsole
			| TerminalKind::Xterm
			| TerminalKind::Generic => {
				cmd.arg("-e").arg(program).args(args);
			}
			TerminalKind::WezTerm => {
				cmd.arg("start").arg("--").arg(program).args(args);
			}
			TerminalKind::Foot => {
				cmd.arg(program).args(args);
			}
			TerminalKind::GnomeTerminal => {
				cmd.arg("--").arg(program).args(args);
			}
		}
		cmd
	}
}

/// Detected terminal information.
#[derive(Debug, Clone)]
pub struct DetectedTerminal {
	/// The terminal kind.
	pub kind: TerminalKind,
	/// Full path to the terminal executable (for debugging).
	#[allow(dead_code)]
	pub path: PathBuf,
}

/// Detects the best available terminal emulator.
///
/// Checks `$TERMINAL` first, then known terminals on PATH (kitty first).
pub fn detect_terminal() -> Option<DetectedTerminal> {
	if let Ok(terminal) = std::env::var("TERMINAL")
		&& let Ok(path) = which::which(&terminal)
	{
		let kind = match terminal.as_str() {
			s if s.contains("kitty") => TerminalKind::Kitty,
			s if s.contains("alacritty") => TerminalKind::Alacritty,
			s if s.contains("wezterm") => TerminalKind::WezTerm,
			s if s.contains("foot") => TerminalKind::Foot,
			s if s.contains("gnome-terminal") => TerminalKind::GnomeTerminal,
			s if s.contains("konsole") => TerminalKind::Konsole,
			s if s.contains("xterm") => TerminalKind::Xterm,
			_ => TerminalKind::Generic,
		};
		return Some(DetectedTerminal { kind, path });
	}

	for kind in KNOWN_TERMINALS {
		if let Ok(path) = which::which(kind.executable()) {
			return Some(DetectedTerminal { kind: *kind, path });
		}
	}

	None
}

/// Spawns xeno in a new terminal window with the given arguments.
pub fn spawn_in_terminal(
	xeno_path: &str,
	args: &[&OsStr],
	socket_path: &str,
) -> std::io::Result<std::process::Child> {
	let terminal = detect_terminal().ok_or_else(|| {
		std::io::Error::new(
			std::io::ErrorKind::NotFound,
			"No terminal emulator found. Set $TERMINAL or install kitty/alacritty/wezterm.",
		)
	})?;

	let mut cmd = terminal.kind.build_command(xeno_path, args);
	cmd.env(super::LOG_SINK_ENV, socket_path);
	cmd.spawn()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_terminal_command_building() {
		let args: &[&OsStr] = &[OsStr::new("test.txt")];

		// Test kitty command
		let cmd = TerminalKind::Kitty.build_command("xeno", args);
		let program = cmd.get_program();
		assert_eq!(program, "kitty");

		// Test wezterm command (different syntax)
		let cmd = TerminalKind::WezTerm.build_command("xeno", args);
		let program = cmd.get_program();
		assert_eq!(program, "wezterm");
	}
}
