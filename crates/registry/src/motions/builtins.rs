//! Built-in motion implementations.

use ropey::RopeSlice;
use xeno_primitives::graphemes::{next_grapheme_boundary, prev_grapheme_boundary};
use xeno_primitives::max_cursor_pos;
use xeno_primitives::range::{CharIdx, Direction, Range};

use crate::motions::movement::{self, WordType, make_range};

// --- Horizontal ---

pub fn move_horizontally(
	text: RopeSlice,
	range: Range,
	direction: Direction,
	count: usize,
	extend: bool,
) -> Range {
	let pos: CharIdx = range.head;
	let max_pos = max_cursor_pos(text);
	let new_pos: CharIdx = match direction {
		Direction::Forward => {
			let mut p = pos;
			for _ in 0..count {
				let next = next_grapheme_boundary(text, p);
				if next > max_pos {
					break;
				}
				p = next;
			}
			p
		}
		Direction::Backward => {
			let mut p = pos;
			for _ in 0..count {
				p = prev_grapheme_boundary(text, p);
			}
			p
		}
	};
	make_range(range, new_pos, extend)
}

motion!(left, { description: "Move left" }, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Backward, count, extend)
});

motion!(right, { description: "Move right" }, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Forward, count, extend)
});

// --- Vertical ---

motion!(up, { description: "Move up" }, |text, range, count, extend| {
	movement::move_vertically(text, range, Direction::Backward, count, extend)
});

motion!(down, { description: "Move down" }, |text, range, count, extend| {
	movement::move_vertically(text, range, Direction::Forward, count, extend)
});

// --- Word ---

motion!(next_word_start, { description: "Move to next word start" }, |text, range, count, extend| {
	movement::move_word(text, range, Direction::Forward, movement::WordBoundary::Start, count, extend)
});

motion!(next_word_end, { description: "Move to next word end" }, |text, range, count, extend| {
	movement::move_word(text, range, Direction::Forward, movement::WordBoundary::End, count, extend)
});

motion!(prev_word_start, { description: "Move to previous word start" }, |text, range, count, extend| {
	movement::move_word(text, range, Direction::Backward, movement::WordBoundary::Start, count, extend)
});

motion!(next_long_word_start, { description: "Move to next WORD start" }, |text, range, count, extend| {
	movement::move_to_next_word_start(text, range, count, WordType::WORD, extend)
});

motion!(prev_long_word_start, { description: "Move to previous WORD start" }, |text, range, count, extend| {
	movement::move_to_prev_word_start(text, range, count, WordType::WORD, extend)
});

motion!(next_long_word_end, { description: "Move to next WORD end" }, |text, range, count, extend| {
	movement::move_to_next_word_end(text, range, count, WordType::WORD, extend)
});

// --- Line ---

motion!(line_start, { description: "Move to line start" }, |text, range, _count, extend| {
	movement::move_to_line_boundary(text, range, movement::LineBoundary::Start, extend)
});

motion!(line_end, { description: "Move to line end" }, |text, range, _count, extend| {
	movement::move_to_line_boundary(text, range, movement::LineBoundary::End, extend)
});

motion!(first_nonwhitespace, { description: "Move to first non-blank character" }, |text, range, _count, extend| {
	movement::move_to_line_boundary(text, range, movement::LineBoundary::FirstNonBlank, extend)
});

// --- Document ---

motion!(document_start, { description: "Move to document start" }, |_text, range, _count, extend| {
	make_range(range, 0, extend)
});

motion!(document_end, { description: "Move to document end" }, |text, range, _count, extend| {
	make_range(range, text.len_chars(), extend)
});

// --- Paragraph ---

motion!(next_paragraph, { description: "Move to next paragraph" }, |text, range, count, extend| {
	movement::move_paragraph(text, range, Direction::Forward, count, extend)
});

motion!(prev_paragraph, { description: "Move to previous paragraph" }, |text, range, count, extend| {
	movement::move_paragraph(text, range, Direction::Backward, count, extend)
});

// --- Diff ---

motion!(next_hunk, { description: "Move to next change" }, |text, range, count, extend| {
	movement::move_to_diff_change(text, range, Direction::Forward, count, extend)
});

motion!(prev_hunk, { description: "Move to previous change" }, |text, range, count, extend| {
	movement::move_to_diff_change(text, range, Direction::Backward, count, extend)
});

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	builder.register_motion(&MOTION_left);
	builder.register_motion(&MOTION_right);
	builder.register_motion(&MOTION_up);
	builder.register_motion(&MOTION_down);
	builder.register_motion(&MOTION_next_word_start);
	builder.register_motion(&MOTION_next_word_end);
	builder.register_motion(&MOTION_prev_word_start);
	builder.register_motion(&MOTION_next_long_word_start);
	builder.register_motion(&MOTION_prev_long_word_start);
	builder.register_motion(&MOTION_next_long_word_end);
	builder.register_motion(&MOTION_line_start);
	builder.register_motion(&MOTION_line_end);
	builder.register_motion(&MOTION_first_nonwhitespace);
	builder.register_motion(&MOTION_document_start);
	builder.register_motion(&MOTION_document_end);
	builder.register_motion(&MOTION_next_paragraph);
	builder.register_motion(&MOTION_prev_paragraph);
	builder.register_motion(&MOTION_next_hunk);
	builder.register_motion(&MOTION_prev_hunk);
}
