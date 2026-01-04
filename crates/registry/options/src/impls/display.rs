//! Display-related options.

use crate::option;

option!(line_numbers, {
	kdl: "line-numbers",
	type: Bool,
	default: true,
	scope: Global,
	description: "Show line numbers in the gutter",
});

option!(wrap_lines, {
	kdl: "wrap-lines",
	type: Bool,
	default: true,
	scope: Buffer,
	description: "Wrap long lines at window edge",
});

option!(cursorline, {
	kdl: "cursorline",
	type: Bool,
	default: true,
	scope: Global,
	description: "Highlight the current line",
});

option!(cursorcolumn, {
	kdl: "cursorcolumn",
	type: Bool,
	default: false,
	scope: Global,
	description: "Highlight the current column",
});

option!(colorcolumn, {
	kdl: "colorcolumn",
	type: Int,
	default: 0,
	scope: Buffer,
	description: "Column to highlight as margin guide",
});

option!(whitespace_visible, {
	kdl: "whitespace-visible",
	type: Bool,
	default: false,
	scope: Global,
	description: "Show whitespace characters",
});
