//! Built-in motion implementations.

use ropey::RopeSlice;
use xeno_primitives::graphemes::{next_grapheme_boundary, prev_grapheme_boundary};
use xeno_primitives::range::{CharIdx, Direction, Range};

use crate::motions::movement::{self, WordType, make_range};

pub fn move_horizontally(
	text: RopeSlice,
	range: Range,
	direction: Direction,
	count: usize,
	extend: bool,
) -> Range {
	let pos: CharIdx = range.head;
	let max_pos = xeno_primitives::rope::max_cell_pos(text).unwrap_or(0);
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

motion_handler!(left, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Backward, count, extend)
});

motion_handler!(right, |text, range, count, extend| {
	move_horizontally(text, range, Direction::Forward, count, extend)
});

motion_handler!(up, |text, range, count, extend| {
	movement::move_vertically(text, range, Direction::Backward, count, extend)
});

motion_handler!(down, |text, range, count, extend| {
	movement::move_vertically(text, range, Direction::Forward, count, extend)
});

motion_handler!(next_word_start, |text, range, count, extend| {
	movement::move_word(
		text,
		range,
		Direction::Forward,
		movement::WordBoundary::Start,
		count,
		extend,
	)
});

motion_handler!(next_word_end, |text, range, count, extend| {
	movement::move_word(
		text,
		range,
		Direction::Forward,
		movement::WordBoundary::End,
		count,
		extend,
	)
});

motion_handler!(prev_word_start, |text, range, count, extend| {
	movement::move_word(
		text,
		range,
		Direction::Backward,
		movement::WordBoundary::Start,
		count,
		extend,
	)
});

motion_handler!(next_long_word_start, |text, range, count, extend| {
	movement::move_to_next_word_start(text, range, count, WordType::WORD, extend)
});

motion_handler!(prev_long_word_start, |text, range, count, extend| {
	movement::move_to_prev_word_start(text, range, count, WordType::WORD, extend)
});

motion_handler!(next_long_word_end, |text, range, count, extend| {
	movement::move_to_next_word_end(text, range, count, WordType::WORD, extend)
});

motion_handler!(line_start, |text, range, _count, extend| {
	movement::move_to_line_boundary(text, range, movement::LineBoundary::Start, extend)
});

motion_handler!(line_end, |text, range, _count, extend| {
	movement::move_to_line_boundary(text, range, movement::LineBoundary::End, extend)
});

motion_handler!(first_nonwhitespace, |text, range, _count, extend| {
	movement::move_to_line_boundary(text, range, movement::LineBoundary::FirstNonBlank, extend)
});

motion_handler!(document_start, |_text, range, _count, extend| {
	make_range(range, 0, extend)
});

motion_handler!(document_end, |text, range, _count, extend| {
	let pos = xeno_primitives::rope::clamp_to_cell(text.len_chars(), text);
	make_range(range, pos, extend)
});

motion_handler!(next_paragraph, |text, range, count, extend| {
	movement::move_paragraph(text, range, Direction::Forward, count, extend)
});

motion_handler!(prev_paragraph, |text, range, count, extend| {
	movement::move_paragraph(text, range, Direction::Backward, count, extend)
});

motion_handler!(next_hunk, |text, range, count, extend| {
	movement::move_to_diff_change(text, range, Direction::Forward, count, extend)
});

motion_handler!(prev_hunk, |text, range, count, extend| {
	movement::move_to_diff_change(text, range, Direction::Backward, count, extend)
});

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	crate::motions::register_compiled(builder);
}

fn register_builtins_reg(
	builder: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), crate::db::builder::RegistryError> {
	register_builtins(builder);
	Ok(())
}

inventory::submit!(crate::db::builtins::BuiltinsReg {
	ordinal: 30,
	f: register_builtins_reg,
});
