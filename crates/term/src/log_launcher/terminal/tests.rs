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
