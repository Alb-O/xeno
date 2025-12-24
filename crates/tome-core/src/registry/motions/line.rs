use crate::motion;
use crate::movement::{move_to_first_nonwhitespace, move_to_line_end, move_to_line_start};

motion!(
	line_start,
	{ description: "Move to line start" },
	|text, range, _count, extend| move_to_line_start(text, range, extend)
);

motion!(
	line_end,
	{ description: "Move to line end" },
	|text, range, _count, extend| move_to_line_end(text, range, extend)
);

motion!(
	first_nonwhitespace,
	{ description: "Move to first non-whitespace" },
	|text, range, _count, extend| move_to_first_nonwhitespace(text, range, extend)
);
