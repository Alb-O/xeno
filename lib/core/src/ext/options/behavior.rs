//! Editor behavior options.

use crate::option;

option!(mouse, Bool, true, Global, "Enable mouse support");
option!(
	line_ending,
	String,
	"lf".to_string(),
	Buffer,
	"Default line ending (lf, crlf, cr)"
);
option!(
	idle_timeout,
	Int,
	250,
	Global,
	"Milliseconds before triggering idle hooks"
);
