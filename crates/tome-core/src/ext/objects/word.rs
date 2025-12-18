use linkme::distributed_slice;
use ropey::RopeSlice;

use crate::ext::{TEXT_OBJECTS, TextObjectDef};
use crate::movement::{WordType, select_word_object};
use crate::range::Range;

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

#[distributed_slice(TEXT_OBJECTS)]
static OBJ_WORD: TextObjectDef = TextObjectDef {
	name: "word",
	trigger: 'w',
	alt_triggers: &[],
	description: "Select word",
	inner: word_inner,
	around: word_around,
};

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

#[distributed_slice(TEXT_OBJECTS)]
static OBJ_WORD_BIG: TextObjectDef = TextObjectDef {
	name: "WORD",
	trigger: 'W',
	alt_triggers: &[],
	description: "Select WORD (non-whitespace)",
	inner: big_word_inner,
	around: big_word_around,
};
