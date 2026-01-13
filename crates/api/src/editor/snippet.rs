use std::ops::Range;

#[derive(Debug, Clone)]
pub struct Snippet {
	pub text: String,
	pub placeholders: Vec<SnippetPlaceholder>,
}

#[derive(Debug, Clone)]
pub struct SnippetPlaceholder {
	pub index: u32,
	pub range: Range<usize>,
}

pub fn parse_snippet(input: &str) -> Option<Snippet> {
	let mut chars = input.chars().peekable();
	let mut text = String::new();
	let mut placeholders = Vec::new();
	let mut out_len = 0usize;

	while let Some(ch) = chars.next() {
		if ch != '$' {
			text.push(ch);
			out_len += 1;
			continue;
		}

		let Some(next) = chars.peek().copied() else {
			text.push('$');
			out_len += 1;
			continue;
		};

		match next {
			'$' => {
				chars.next();
				text.push('$');
				out_len += 1;
			}
			'0'..='9' => {
				let index = parse_index(&mut chars)?;
				placeholders.push(SnippetPlaceholder {
					index,
					range: out_len..out_len,
				});
			}
			'{' => {
				chars.next();
				let index = parse_index(&mut chars)?;
				match chars.peek().copied() {
					Some(':') => {
						chars.next();
						let start = out_len;
						let default = parse_default_text(&mut chars)?;
						text.push_str(&default);
						out_len += default.chars().count();
						placeholders.push(SnippetPlaceholder {
							index,
							range: start..out_len,
						});
					}
					Some('}') => {
						chars.next();
						placeholders.push(SnippetPlaceholder {
							index,
							range: out_len..out_len,
						});
					}
					_ => return None,
				}
			}
			_ => {
				text.push('$');
				out_len += 1;
			}
		}
	}

	Some(Snippet { text, placeholders })
}

fn parse_index(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<u32> {
	let mut digits = String::new();
	while let Some(ch) = chars.peek().copied()
		&& ch.is_ascii_digit()
	{
		digits.push(ch);
		chars.next();
	}
	if digits.is_empty() {
		return None;
	}
	digits.parse().ok()
}

fn parse_default_text(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
	let mut text = String::new();
	let mut closed = false;
	while let Some(ch) = chars.next() {
		match ch {
			'}' => {
				closed = true;
				break;
			}
			'\\' => {
				let Some(escaped) = chars.next() else {
					return None;
				};
				text.push(escaped);
			}
			_ => text.push(ch),
		}
	}
	if closed {
		Some(text)
	} else {
		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn snippet_parses_simple_placeholders() {
		let snippet = parse_snippet("foo $1 bar $0").unwrap();
		assert_eq!(snippet.text, "foo  bar ");
		assert_eq!(snippet.placeholders.len(), 2);
		assert_eq!(snippet.placeholders[0].index, 1);
		assert_eq!(snippet.placeholders[1].index, 0);
	}

	#[test]
	fn snippet_parses_default_text() {
		let snippet = parse_snippet("let ${1:name} = ${2:val};").unwrap();
		assert_eq!(snippet.text, "let name = val;");
		assert_eq!(snippet.placeholders.len(), 2);
		assert_eq!(snippet.placeholders[0].range, 4..8);
		assert_eq!(snippet.placeholders[1].range, 11..14);
	}
}
