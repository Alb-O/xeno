//! Snapshot-based host implementation for Nu macro/hook evaluation.
//!
//! Captures buffer metadata and rope content at invocation time so the host
//! can be moved to the worker thread without borrowing editor state.

use xeno_nu_api::{BufferMeta, HostError, LineColRange, TextChunk, XenoNuHost};
use xeno_primitives::Rope;

/// Owned snapshot of a buffer's state, captured before dispatching to the Nu worker.
pub(crate) struct NuHostSnapshot {
	meta: BufferMeta,
	rope: Rope,
}

impl NuHostSnapshot {
	pub(crate) fn new(meta: BufferMeta, rope: Rope) -> Self {
		Self { meta, rope }
	}
}

impl XenoNuHost for NuHostSnapshot {
	fn buffer_get(&self, id: Option<i64>) -> Result<BufferMeta, HostError> {
		if id.is_some() {
			return Err(HostError("cross-buffer queries are not yet supported".into()));
		}
		Ok(self.meta.clone())
	}

	fn buffer_text(&self, id: Option<i64>, range: Option<LineColRange>, max_bytes: usize) -> Result<TextChunk, HostError> {
		if id.is_some() {
			return Err(HostError("cross-buffer queries are not yet supported".into()));
		}

		let text = &self.rope;
		let (start_char, end_char) = match range {
			Some(r) => {
				let line_count = text.len_lines();
				if r.start_line >= line_count {
					return Ok(TextChunk {
						text: String::new(),
						truncated: false,
					});
				}
				let end_line = r.end_line.min(line_count.saturating_sub(1));

				let start_line_char = text.line_to_char(r.start_line);
				let start_line_len = text.line(r.start_line).len_chars();
				let sc = start_line_char + r.start_col.min(start_line_len);

				let end_line_char = text.line_to_char(end_line);
				let end_line_len = text.line(end_line).len_chars();
				let ec = end_line_char + r.end_col.min(end_line_len);

				(sc, ec.max(sc))
			}
			None => (0, text.len_chars()),
		};

		// Build string by iterating rope chunks, stopping at max_bytes.
		let slice = text.slice(start_char..end_char);
		let mut result = String::new();
		let mut remaining = max_bytes;
		let mut truncated = false;

		for chunk in slice.chunks() {
			if remaining == 0 {
				truncated = true;
				break;
			}
			if chunk.len() <= remaining {
				result.push_str(chunk);
				remaining -= chunk.len();
			} else {
				// Find last valid UTF-8 char boundary within budget
				let mut end = remaining;
				while end > 0 && !chunk.is_char_boundary(end) {
					end -= 1;
				}
				result.push_str(&chunk[..end]);
				truncated = true;
				break;
			}
		}

		Ok(TextChunk { text: result, truncated })
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn test_meta() -> BufferMeta {
		BufferMeta {
			path: Some("/tmp/test.rs".into()),
			file_type: Some("rust".into()),
			readonly: false,
			modified: false,
			line_count: 0,
		}
	}

	#[test]
	fn utf8_truncation_inside_emoji() {
		// "a🙂b" = a(1) + 🙂(4) + b(1) = 6 bytes
		let rope = Rope::from("a🙂b");
		let host = NuHostSnapshot::new(test_meta(), rope);
		// max_bytes=3 cuts inside the emoji → must back up to "a"
		let chunk = host.buffer_text(None, None, 3).unwrap();
		assert_eq!(chunk.text, "a");
		assert!(chunk.truncated);
		// Verify it's valid UTF-8 (would panic on invalid)
		let _ = chunk.text.as_bytes();
	}

	#[test]
	fn ranged_extraction_single_line() {
		let rope = Rope::from("line zero\nline one\nline two\n");
		let host = NuHostSnapshot::new(test_meta(), rope);
		let range = LineColRange {
			start_line: 1,
			start_col: 0,
			end_line: 1,
			end_col: usize::MAX,
		};
		let chunk = host.buffer_text(None, Some(range), 1024).unwrap();
		assert_eq!(chunk.text, "line one\n");
		assert!(!chunk.truncated);
	}

	#[test]
	fn ranged_extraction_across_lines() {
		let rope = Rope::from("aaa\nbbb\nccc\n");
		let host = NuHostSnapshot::new(test_meta(), rope);
		let range = LineColRange {
			start_line: 0,
			start_col: 0,
			end_line: 1,
			end_col: usize::MAX,
		};
		let chunk = host.buffer_text(None, Some(range), 1024).unwrap();
		assert_eq!(chunk.text, "aaa\nbbb\n");
		assert!(!chunk.truncated);
	}

	#[test]
	fn id_rejection() {
		let rope = Rope::from("hello");
		let host = NuHostSnapshot::new(test_meta(), rope);
		let err = host.buffer_get(Some(42)).unwrap_err();
		assert!(err.0.contains("cross-buffer"));
		let err = host.buffer_text(Some(42), None, 1024).unwrap_err();
		assert!(err.0.contains("cross-buffer"));
	}

	#[test]
	fn full_text_no_truncation() {
		let rope = Rope::from("hello world");
		let host = NuHostSnapshot::new(test_meta(), rope);
		let chunk = host.buffer_text(None, None, 1024).unwrap();
		assert_eq!(chunk.text, "hello world");
		assert!(!chunk.truncated);
	}

	#[test]
	fn out_of_range_start_line_returns_empty() {
		let rope = Rope::from("only one line");
		let host = NuHostSnapshot::new(test_meta(), rope);
		let range = LineColRange {
			start_line: 100,
			start_col: 0,
			end_line: 200,
			end_col: 0,
		};
		let chunk = host.buffer_text(None, Some(range), 1024).unwrap();
		assert_eq!(chunk.text, "");
		assert!(!chunk.truncated);
	}
}
