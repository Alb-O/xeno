//! Argument/parameter text object.

use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::ext::{TEXT_OBJECTS, TextObjectDef};
use crate::range::Range;

/// Find argument boundaries, handling nested delimiters and whitespace.
///
/// Returns: (start, content_start, content_end, end)
/// - `start`: Position including leading whitespace/comma
/// - `content_start`: Position after trimming leading whitespace
/// - `content_end`: Position before trailing whitespace
/// - `end`: Position including trailing comma (for "around" selection)
///
/// The function properly handles:
/// - Nested parentheses, brackets, and braces
/// - Leading/trailing whitespace trimming for "inner" selection
/// - Comma inclusion for "around" selection
fn find_arg_boundaries(text: RopeSlice, pos: usize) -> Option<(usize, usize, usize, usize)> {
	let len = text.len_chars();
	if len == 0 {
		return None;
	}

	let mut depth = 0i32;
	let mut start = pos;
	let mut content_start = pos;

	// Search backward for argument start
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
				// Skip whitespace after comma
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

	// Search forward for argument end
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

fn arg_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	let (_, content_start, content_end, _) = find_arg_boundaries(text, pos)?;
	Some(Range::new(content_start, content_end))
}

fn arg_around(text: RopeSlice, pos: usize) -> Option<Range> {
	let (start, _, _, end) = find_arg_boundaries(text, pos)?;
	Some(Range::new(start, end))
}

#[distributed_slice(TEXT_OBJECTS)]
static OBJ_ARGUMENT: TextObjectDef = TextObjectDef {
	name: "argument",
	trigger: 'c',
	alt_triggers: &[],
	description: "Select function argument",
	inner: arg_inner,
	around: arg_around,
};
