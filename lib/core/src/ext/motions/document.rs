use crate::motion;
use crate::movement::{self, move_to_document_end, move_to_document_start};

motion!(
	document_start,
	"Move to document start",
	|text, range, _count, extend| { move_to_document_start(text, range, extend) }
);

motion!(
	document_end,
	"Move to document end",
	|text, range, _count, extend| { move_to_document_end(text, range, extend) }
);

motion!(
	find_char_forward,
	"Find character forward (placeholder)",
	|text, range, _count, extend| { movement::make_range(range, range.head, extend) }
);
