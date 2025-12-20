//! Scrolling behavior options.

use crate::option;

option!(
	scroll_margin,
	Int,
	3,
	Global,
	"Minimum lines to keep above/below cursor when scrolling"
);
option!(
	scroll_smooth,
	Bool,
	false,
	Global,
	"Enable smooth scrolling animations"
);
