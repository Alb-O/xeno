use crate::motion;
use crate::movement::{
	move_to_next_word_end, move_to_next_word_start, move_to_prev_word_start, WordType,
};

motion!(
	next_word_start,
	{ description: "Move to next word start" },
	|text, range, count, extend| {
		move_to_next_word_start(text, range, count, WordType::Word, extend)
	}
);

motion!(
	prev_word_start,
	{ description: "Move to previous word start" },
	|text, range, count, extend| {
		move_to_prev_word_start(text, range, count, WordType::Word, extend)
	}
);

motion!(
	next_word_end,
	{ description: "Move to next word end" },
	|text, range, count, extend| {
		move_to_next_word_end(text, range, count, WordType::Word, extend)
	}
);

motion!(
	next_WORD_start,
	{ description: "Move to next WORD start" },
	|text, range, count, extend| {
		move_to_next_word_start(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	next_long_word_start,
	{ description: "Move to next WORD start" },
	|text, range, count, extend| {
		move_to_next_word_start(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	prev_WORD_start,
	{ description: "Move to previous WORD start" },
	|text, range, count, extend| {
		move_to_prev_word_start(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	prev_long_word_start,
	{ description: "Move to previous WORD start" },
	|text, range, count, extend| {
		move_to_prev_word_start(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	next_WORD_end,
	{ description: "Move to next WORD end" },
	|text, range, count, extend| {
		move_to_next_word_end(text, range, count, WordType::WORD, extend)
	}
);

motion!(
	next_long_word_end,
	{ description: "Move to next WORD end" },
	|text, range, count, extend| {
		move_to_next_word_end(text, range, count, WordType::WORD, extend)
	}
);
