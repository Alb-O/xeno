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
/// * `action:<name>` — no whitespace allowed in name, no args
/// * `command:<name> [args...]` — shell-like quoting for args
/// * `editor:<name> [args...]` — shell-like quoting for args
/// * `nu:<name> [args...]` — shell-like quoting for args
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
mod tests;
