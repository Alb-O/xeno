use super::CommandPaletteOverlay;

#[test]
fn token_context_switches_to_arg_after_space() {
	let token = CommandPaletteOverlay::token_context("theme ", 6);
	assert_eq!(token.cmd, "theme");
	assert_eq!(token.token_index, 1);
	assert_eq!(token.start, 6);
	assert_eq!(token.query, "");
}

#[test]
fn token_context_preserves_path_prefix_for_replacement_start() {
	let token = CommandPaletteOverlay::token_context("open src/ma", 11);
	assert_eq!(token.cmd, "open");
	assert_eq!(token.token_index, 1);
	assert_eq!(token.path_dir.as_deref(), Some("src/"));
	assert_eq!(token.query, "ma");
	assert_eq!(token.start, 9);
}

#[test]
fn token_context_handles_quoted_path_argument() {
	let token = CommandPaletteOverlay::token_context("open \"My Folder/ma\"", 18);
	assert_eq!(token.cmd, "open");
	assert_eq!(token.token_index, 1);
	assert_eq!(token.quoted, Some('"'));
	assert_eq!(token.path_dir.as_deref(), Some("My Folder/"));
	assert_eq!(token.query, "ma");
}

#[test]
fn replace_char_range_is_char_index_safe() {
	let (out, cursor) = CommandPaletteOverlay::replace_char_range("ab界d", 1, 3, "ZZ");
	assert_eq!(out, "aZZd");
	assert_eq!(cursor, 3);
}

#[test]
fn effective_replace_end_avoids_deleting_closing_quote() {
	let input = "open \"My Folder/ma\"";
	let cursor = CommandPaletteOverlay::char_count(input);
	let token = CommandPaletteOverlay::token_context(input, cursor);
	let replace_end = CommandPaletteOverlay::effective_replace_end(&token, cursor);
	assert_eq!(token.close_quote_idx, Some(replace_end));
}

#[test]
fn command_items_prioritize_exact_alias_match() {
	let usage = crate::completion::CommandUsageSnapshot::default();
	let items = CommandPaletteOverlay::build_command_items("w", &usage);
	assert_eq!(items.first().map(|item| item.label.as_str()), Some("write"));
}

#[test]
fn command_items_include_files_picker_command() {
	let usage = crate::completion::CommandUsageSnapshot::default();
	let items = CommandPaletteOverlay::build_command_items("fi", &usage);
	assert!(items.iter().any(|item| item.label == "files"));
}

#[test]
fn command_space_policy_is_disabled_for_no_arg_commands() {
	assert!(!CommandPaletteOverlay::command_supports_argument_completion("quit"));
	assert!(!CommandPaletteOverlay::command_supports_argument_completion("wq"));
}

#[test]
fn command_space_policy_is_enabled_for_commands_with_arg_completion() {
	assert!(CommandPaletteOverlay::command_supports_argument_completion("theme"));
	assert!(CommandPaletteOverlay::command_supports_argument_completion("edit"));
	assert!(CommandPaletteOverlay::command_supports_argument_completion("snippet"));
}

#[test]
fn command_space_policy_resolves_aliases() {
	assert!(CommandPaletteOverlay::command_supports_argument_completion("e"));
	assert!(CommandPaletteOverlay::command_supports_argument_completion("snip"));
}
