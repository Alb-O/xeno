use super::*;

#[test]
fn split_simple() {
	assert_eq!(split_invocation_args("write file.txt").unwrap(), vec!["write", "file.txt"]);
}

#[test]
fn split_double_quoted_spaces() {
	assert_eq!(split_invocation_args(r#"write "foo bar.txt""#).unwrap(), vec!["write", "foo bar.txt"]);
}

#[test]
fn split_single_quoted() {
	assert_eq!(split_invocation_args("open 'a b' c").unwrap(), vec!["open", "a b", "c"]);
}

#[test]
fn split_double_quote_escapes() {
	assert_eq!(
		split_invocation_args(r#"git commit -m "hello \"world\"""#).unwrap(),
		vec!["git", "commit", "-m", r#"hello "world""#]
	);
}

#[test]
fn split_backslash_space() {
	assert_eq!(split_invocation_args(r"write foo\ bar.txt").unwrap(), vec!["write", "foo bar.txt"]);
}

#[test]
fn split_unterminated_double_quote() {
	assert!(split_invocation_args(r#"write "unterminated"#).is_err());
}

#[test]
fn split_unterminated_single_quote() {
	assert!(split_invocation_args("write 'unterminated").is_err());
}

#[test]
fn split_trailing_backslash() {
	assert!(split_invocation_args(r"write \").is_err());
}

#[test]
fn split_empty() {
	assert!(split_invocation_args("").unwrap().is_empty());
}

#[test]
fn parse_command_quoted_args() {
	let spec = parse_spec(r#"command:write "foo bar.txt""#).unwrap();
	assert_eq!(spec.kind, SpecKind::Command);
	assert_eq!(spec.name, "write");
	assert_eq!(spec.args, vec!["foo bar.txt"]);
}

#[test]
fn parse_editor_quoted_args() {
	let spec = parse_spec(r#"editor:open "a b" c"#).unwrap();
	assert_eq!(spec.kind, SpecKind::Editor);
	assert_eq!(spec.name, "open");
	assert_eq!(spec.args, vec!["a b", "c"]);
}

#[test]
fn parse_command_preserves_quoted_arg() {
	let spec = parse_spec(r#"command:git commit -m "hello world""#).unwrap();
	assert_eq!(spec.kind, SpecKind::Command);
	assert_eq!(spec.name, "git");
	assert_eq!(spec.args, vec!["commit", "-m", "hello world"]);
}

#[test]
fn parse_action_no_spaces() {
	let spec = parse_spec("action:move_right").unwrap();
	assert_eq!(spec.kind, SpecKind::Action);
	assert_eq!(spec.name, "move_right");
	assert!(spec.args.is_empty());
}

#[test]
fn parse_action_rejects_spaces() {
	assert!(parse_spec("action:move right").is_err());
}

#[test]
fn parse_empty_rejects() {
	assert!(parse_spec("").is_err());
}

#[test]
fn parse_unknown_prefix_rejects() {
	assert!(parse_spec("foo:bar").is_err());
}

#[test]
fn parse_nu() {
	let spec = parse_spec("nu:go").unwrap();
	assert_eq!(spec.kind, SpecKind::Nu);
	assert_eq!(spec.name, "go");
	assert!(spec.args.is_empty());
}

#[test]
fn parse_nu_with_args() {
	let spec = parse_spec(r#"nu:go "a b" c"#).unwrap();
	assert_eq!(spec.kind, SpecKind::Nu);
	assert_eq!(spec.name, "go");
	assert_eq!(spec.args, vec!["a b", "c"]);
}

#[test]
fn parse_nu_rejects_invalid_name() {
	assert!(parse_spec("nu:bad/name").is_err());
	assert!(parse_spec("nu:").is_err());
}
