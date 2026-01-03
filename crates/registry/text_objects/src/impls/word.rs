//! Word text objects.

use ropey::RopeSlice;
use xeno_base::Range;

use crate::movement::{WordType, select_word_object};
use crate::text_object;

text_object!(
	word,
	{ trigger: 'w', description: "Select word" },
	{
		inner: word_inner,
		around: word_around,
	}
);

/// Selects the inner word (alphanumeric characters only).
fn word_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	Some(select_word_object(
		text,
		Range::point(pos),
		WordType::Word,
		true,
	))
}

/// Selects the word including surrounding whitespace.
fn word_around(text: RopeSlice, pos: usize) -> Option<Range> {
	Some(select_word_object(
		text,
		Range::point(pos),
		WordType::Word,
		false,
	))
}

text_object!(
	WORD,
	{ trigger: 'W', description: "Select WORD (non-whitespace)" },
	{
		inner: big_word_inner,
		around: big_word_around,
	}
);

/// Selects the inner WORD (any non-whitespace characters).
fn big_word_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	Some(select_word_object(
		text,
		Range::point(pos),
		WordType::WORD,
		true,
	))
}

/// Selects the WORD including surrounding whitespace.
fn big_word_around(text: RopeSlice, pos: usize) -> Option<Range> {
	Some(select_word_object(
		text,
		Range::point(pos),
		WordType::WORD,
		false,
	))
}
