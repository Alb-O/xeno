//! Display-related options.

use crate::option;

option!(
	line_numbers,
	Bool,
	true,
	Global,
	"Show line numbers in the gutter"
);
option!(
	wrap_lines,
	Bool,
	true,
	Buffer,
	"Wrap long lines at window edge"
);
option!(cursorline, Bool, true, Global, "Highlight the current line");
option!(
	cursorcolumn,
	Bool,
	false,
	Global,
	"Highlight the current column"
);
option!(
	colorcolumn,
	Int,
	0,
	Buffer,
	"Column to highlight as margin guide"
);
option!(
	whitespace_visible,
	Bool,
	false,
	Global,
	"Show whitespace characters"
);
