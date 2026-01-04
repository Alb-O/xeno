//! Scrolling behavior options.

use crate::option;

option!(scroll_margin, {
	kdl: "scroll-margin",
	type: Int,
	default: 3,
	scope: Global,
	description: "Minimum lines to keep above/below cursor when scrolling",
});

option!(scroll_smooth, {
	kdl: "scroll-smooth",
	type: Bool,
	default: false,
	scope: Global,
	description: "Enable smooth scrolling animations",
});
