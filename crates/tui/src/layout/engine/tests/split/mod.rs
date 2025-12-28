//! Tests for the `Layout::split()` function.
//!
//! There are many tests in this as the number of edge cases that are caused by the interaction
//! between the constraints is quite large. The tests are split into sections based on the type
//! of constraints that are used.
//!
//! These tests are characterization tests. This means that they are testing the way the code
//! currently works, and not the way it should work. This is because the current behavior is not
//! well defined, and it is not clear what the correct behavior should be. This means that if
//! the behavior changes, these tests should be updated to match the new behavior.
//!
//!  EOL comments in each test are intended to communicate the purpose of each test and to make
//!  it easy to see that the tests are as exhaustive as feasible:
//! - zero: constraint is zero
//! - exact: constraint is equal to the space
//! - underflow: constraint is for less than the full space
//! - overflow: constraint is for more than the full space

use alloc::string::ToString;

use pretty_assertions::assert_eq;

use crate::buffer::Buffer;
use crate::layout::{Constraint, Direction, Flex, Layout, Rect};
use crate::text::Text;
use crate::widgets::Widget;

/// Test that the given constraints applied to the given area result in the expected layout.
/// Each chunk is filled with a letter repeated as many times as the width of the chunk. The
/// resulting buffer is compared to the expected string.
///
/// This approach is used rather than testing the resulting rects directly because it is
/// easier to visualize the result, and it leads to more concise tests that are easier to
/// compare against each other. E.g. `"abc"` is much more concise than `[Rect::new(0, 0, 1,
/// 1), Rect::new(1, 0, 1, 1), Rect::new(2, 0, 1, 1)]`.
#[track_caller]
pub(super) fn letters(flex: Flex, constraints: &[Constraint], width: u16, expected: &str) {
	let area = Rect::new(0, 0, width, 1);
	let layout = Layout::default()
		.direction(Direction::Horizontal)
		.constraints(constraints)
		.flex(flex)
		.split(area);
	let mut buffer = Buffer::empty(area);
	for (c, &area) in ('a'..='z').take(constraints.len()).zip(layout.iter()) {
		let s = c.to_string().repeat(area.width as usize);
		Text::from(s).render(area, &mut buffer);
	}
	assert_eq!(buffer, Buffer::with_lines([expected]));
}

mod constraints;
mod fill;
mod flex;
mod percentage;
mod ratio;
mod spacers;
