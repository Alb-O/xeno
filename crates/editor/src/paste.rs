//! Text normalization helpers for terminal and file-input paste paths.

/// Normalizes text to LF (`\n`) newlines.
///
/// Converts both CRLF (`\r\n`) and bare CR (`\r`) into LF.
/// If the input contains no carriage returns, the original string is returned.
pub fn normalize_to_lf(mut s: String) -> String {
	if !s.contains('\r') {
		return s;
	}

	let mut out = String::with_capacity(s.len());
	let mut chars = s.drain(..).peekable();
	while let Some(ch) = chars.next() {
		if ch == '\r' {
			if chars.peek() == Some(&'\n') {
				chars.next();
			}
			out.push('\n');
		} else {
			out.push(ch);
		}
	}

	out
}

#[cfg(test)]
mod tests {
	use super::normalize_to_lf;

	#[test]
	fn crlf_to_lf() {
		assert_eq!(normalize_to_lf("a\r\nb\r\n".to_string()), "a\nb\n");
	}

	#[test]
	fn cr_to_lf() {
		assert_eq!(normalize_to_lf("a\rb\rc".to_string()), "a\nb\nc");
	}

	#[test]
	fn mixed_sequences() {
		assert_eq!(normalize_to_lf("a\r\nb\rc\n".to_string()), "a\nb\nc\n");
	}

	#[test]
	fn idempotent() {
		let input = "a\r\nb\rc\n".to_string();
		let once = normalize_to_lf(input);
		let twice = normalize_to_lf(once.clone());
		assert_eq!(twice, once);
	}
}
