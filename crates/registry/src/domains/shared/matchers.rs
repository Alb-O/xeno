//! Shared matcher utilities for selector-based domain queries.

/// Matches a path-aware glob pattern.
pub fn glob_matches(pattern: &str, path: &str, filename: Option<&str>) -> bool {
	if !pattern.contains('/') {
		return filename.is_some_and(|f| glob_match_simple(pattern, f));
	}
	glob_match_simple(pattern, path)
}

fn glob_match_simple(pattern: &str, text: &str) -> bool {
	let mut p = pattern.chars().peekable();
	let mut t = text.chars().peekable();

	while let Some(pc) = p.next() {
		match pc {
			'*' => {
				if p.peek() == Some(&'*') {
					p.next();
					let remaining: String = p.collect();
					if remaining.is_empty() {
						return true;
					}
					let rest: String = t.collect();
					return (0..=rest.len()).any(|i| glob_match_simple(&remaining, &rest[i..]));
				}

				let remaining: String = p.collect();
				if remaining.is_empty() {
					return !t.any(|c| c == '/');
				}

				let rest: String = t.collect();
				for (i, c) in rest.char_indices() {
					if c == '/' {
						break;
					}
					if glob_match_simple(&remaining, &rest[i..]) {
						return true;
					}
				}
				return glob_match_simple(&remaining, "");
			}
			'?' if t.next().is_none() => return false,
			'?' => {}
			c if t.next() != Some(c) => return false,
			_ => {}
		}
	}

	t.next().is_none()
}
