//! Indentation-related options.

use crate::option;

option!(
	tab_width,
	Int,
	4,
	Buffer,
	"Width of a tab character for display"
);
option!(
	indent_width,
	Int,
	4,
	Buffer,
	"Number of spaces for each indent level"
);
option!(
	use_tabs,
	Bool,
	false,
	Buffer,
	"Use tabs instead of spaces for indentation"
);
