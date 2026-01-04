//! Editor behavior options.

use crate::option;

option!(mouse, {
	kdl: "mouse",
	type: Bool,
	default: true,
	scope: Global,
	description: "Enable mouse support",
});

option!(line_ending, {
	kdl: "line-ending",
	type: String,
	default: "lf".to_string(),
	scope: Buffer,
	description: "Default line ending (lf, crlf, cr)",
});

option!(idle_timeout, {
	kdl: "idle-timeout",
	type: Int,
	default: 250,
	scope: Global,
	description: "Milliseconds before triggering idle hooks",
});
