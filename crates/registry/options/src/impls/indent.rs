//! Indentation-related options.

use crate::option;

option!(tab_width, {
	kdl: "tab-width",
	type: Int,
	default: 4,
	scope: Buffer,
	description: "Number of spaces a tab character occupies for display",
});

option!(indent_width, {
	kdl: "indent-width",
	type: Int,
	default: 4,
	scope: Buffer,
	description: "Number of spaces per indentation level",
});

option!(use_tabs, {
	kdl: "use-tabs",
	type: Bool,
	default: false,
	scope: Buffer,
	description: "Use tabs instead of spaces for indentation",
});
