//! Invocation spec parser for `action:`, `command:`, and `editor:` target strings.
//!
//! Provides [`parse_spec`] which tokenizes a spec string with shell-like quoting
//! and returns a [`ParsedSpec`] with kind, name, and arguments.

/// The kind of invocation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecKind {
	Action,
	Command,
	Editor,
	Nu,
}

/// A parsed invocation spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSpec {
	pub kind: SpecKind,
	pub name: String,
	pub args: Vec<String>,
}

/// Parse an invocation spec string into its components.
///
/// Accepted formats:
/// - `action:<name>` — no whitespace allowed in name, no args
/// - `command:<name> [args...]` — shell-like quoting for args
/// - `editor:<name> [args...]` — shell-like quoting for args
/// - `nu:<name> [args...]` — shell-like quoting for args
pub fn parse_spec(spec: &str) -> Result<ParsedSpec, String> {
	let spec = spec.trim();
	if spec.is_empty() {
		return Err("empty invocation spec".to_string());
	}

	if let Some(action) = spec.strip_prefix("action:") {
		let action = action.trim();
		if action.is_empty() {
			return Err("action invocation missing target".to_string());
		}
		if action.contains(char::is_whitespace) {
			return Err(format!("action invocation must not include spaces: {spec}"));
		}
		return Ok(ParsedSpec {
			kind: SpecKind::Action,
			name: action.to_string(),
			args: Vec::new(),
		});
	}

	if let Some(rest) = spec.strip_prefix("command:") {
		let tokens = split_invocation_args(rest)?;
		let name = tokens.first().ok_or("command invocation missing command name")?.clone();
		let args = tokens[1..].to_vec();
		return Ok(ParsedSpec {
			kind: SpecKind::Command,
			name,
			args,
		});
	}

	if let Some(rest) = spec.strip_prefix("editor:") {
		let tokens = split_invocation_args(rest)?;
		let name = tokens.first().ok_or("editor invocation missing command name")?.clone();
		let args = tokens[1..].to_vec();
		return Ok(ParsedSpec {
			kind: SpecKind::Editor,
			name,
			args,
		});
	}

	if let Some(rest) = spec.strip_prefix("nu:") {
		let tokens = split_invocation_args(rest)?;
		let name = tokens.first().ok_or("nu invocation missing function name")?.clone();
		if !is_valid_nu_function_name(&name) {
			return Err(format!("nu invocation function name contains unsupported characters: {name}"));
		}
		let args = tokens[1..].to_vec();
		return Ok(ParsedSpec {
			kind: SpecKind::Nu,
			name,
			args,
		});
	}

	Err(format!("unsupported invocation spec '{spec}', expected action:/command:/editor:/nu:"))
}

fn is_valid_nu_function_name(name: &str) -> bool {
	!name.is_empty() && name.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}

/// Tokenize an argument string with shell-like quoting.
///
/// Splits on unquoted whitespace. Supports double-quoted strings with
/// `\"`, `\\`, `\n`, `\t`, `\r` escapes, single-quoted strings with no
/// escapes, and backslash-space outside quotes.
pub fn split_invocation_args(input: &str) -> Result<Vec<String>, String> {
	let input = input.trim();
	if input.is_empty() {
		return Ok(Vec::new());
	}

	let mut tokens = Vec::new();
	let mut current = String::new();
	let mut chars = input.chars().peekable();

	while let Some(&ch) = chars.peek() {
		match ch {
			' ' | '\t' => {
				if !current.is_empty() {
					tokens.push(std::mem::take(&mut current));
				}
				chars.next();
			}
			'"' => {
				chars.next();
				loop {
					match chars.next() {
						None => return Err("unterminated double quote".to_string()),
						Some('"') => break,
						Some('\\') => match chars.next() {
							None => return Err("trailing backslash in double quote".to_string()),
							Some('"') => current.push('"'),
							Some('\\') => current.push('\\'),
							Some('n') => current.push('\n'),
							Some('t') => current.push('\t'),
							Some('r') => current.push('\r'),
							Some(c) => {
								current.push('\\');
								current.push(c);
							}
						},
						Some(c) => current.push(c),
					}
				}
			}
			'\'' => {
				chars.next();
				loop {
					match chars.next() {
						None => return Err("unterminated single quote".to_string()),
						Some('\'') => break,
						Some(c) => current.push(c),
					}
				}
			}
			'\\' => {
				chars.next();
				match chars.next() {
					None => return Err("trailing backslash".to_string()),
					Some(c) => current.push(c),
				}
			}
			_ => {
				current.push(ch);
				chars.next();
			}
		}
	}

	if !current.is_empty() {
		tokens.push(current);
	}

	Ok(tokens)
}

#[cfg(test)]
mod tests {
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
}
