use crate::motion;
use crate::movement::{move_to_next_paragraph, move_to_prev_paragraph};

motion!(
	next_paragraph,
	{ description: "Move to next paragraph" },
	|text, range, count, extend| move_to_next_paragraph(text, range, count, extend)
);

motion!(
	prev_paragraph,
	{ description: "Move to previous paragraph" },
	|text, range, count, extend| move_to_prev_paragraph(text, range, count, extend)
);
