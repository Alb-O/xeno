use ropey::RopeSlice;

use crate::movement::{WordType, select_word_object};
use crate::range::Range;
use crate::text_object;

text_object!(
	word,
	{ trigger: 'w', description: "Select word" },
	{
		inner: word_inner,
		around: word_around,
	}
);

fn word_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	Some(select_word_object(
		text,
		Range::point(pos),
		WordType::Word,
		true,
	))
}

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

fn big_word_inner(text: RopeSlice, pos: usize) -> Option<Range> {
	Some(select_word_object(
		text,
		Range::point(pos),
		WordType::WORD,
		true,
	))
}

fn big_word_around(text: RopeSlice, pos: usize) -> Option<Range> {
	Some(select_word_object(
		text,
		Range::point(pos),
		WordType::WORD,
		false,
	))
}
