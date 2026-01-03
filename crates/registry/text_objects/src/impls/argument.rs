//! Argument/parameter text object.

use ropey::RopeSlice;
use xeno_base::Range;

use crate::text_object;

/// Finds the boundaries of a function argument at the given position.
fn find_arg_boundaries(text: RopeSlice, pos: usize) -> Option<(usize, usize, usize, usize)> {
	let len = text.len_chars();
	if len == 0 {
		return None;
	}

	let mut depth = 0i32;
	let mut start = pos;
	let mut content_start = pos;

	for i in (0..pos).rev() {
		let ch = text.char(i);
		match ch {
			')' | ']' | '}' => depth += 1,
			'(' | '[' | '{' => {
				if depth == 0 {
					start = i + 1;
					content_start = i + 1;
					break;
				}
				depth -= 1;
			}
			',' if depth == 0 => {
				start = i + 1;
				content_start = i + 1;
				while content_start < pos && text.char(content_start).is_whitespace() {
					content_start += 1;
				}
				break;
			}
			_ => {}
		}
	}

	depth = 0;
	let mut end = pos;
	let mut content_end = pos;

	for i in pos..len {
		let ch = text.char(i);
		match ch {
			'(' | '[' | '{' => depth += 1,
			')' | ']' | '}' => {
				if depth == 0 {
					end = i;
					content_end = i;
					while content_end > start && text.char(content_end - 1).is_whitespace() {
						content_end -= 1;
					}
					break;
				}
				depth -= 1;
			}
			',' if depth == 0 => {
				content_end = i;
				end = i + 1;
				while content_end > start && text.char(content_end - 1).is_whitespace() {
					content_end -= 1;
				}
				break;
			}
			_ => {
				end = i + 1;
				content_end = i + 1;
			}
		}
	}

	Some((start, content_start, content_end, end))
}

/// Selects the inner content of an argument (excluding surrounding whitespace/comma).
fn arg_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	let (_, content_start, content_end, _) = find_arg_boundaries(text, pos)?;
	Some(Range::new(content_start, content_end))
}

/// Selects the argument including surrounding whitespace/comma.
fn arg_around(text: RopeSlice, pos: usize) -> Option<Range> {
	let (start, _, _, end) = find_arg_boundaries(text, pos)?;
	Some(Range::new(start, end))
}

text_object!(
	argument,
	{ trigger: 'c', description: "Select function argument" },
	{
		inner: arg_inner,
		around: arg_around,
	}
);
