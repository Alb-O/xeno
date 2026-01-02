//! Panel definitions and runtime management.
//!
//! This module defines the built-in panels (terminal, debug) using the [`panel!`] macro
//! with inline factories, and provides [`PanelRegistry`] for runtime instance management.

mod registry;

use evildoer_registry::panel;
pub use registry::PanelRegistry;

use crate::debug::DebugPanel;
use crate::terminal::TerminalBuffer;

panel!(terminal, {
	description: "Embedded terminal emulator",
	mode_name: "TERMINAL",
	layer: 1,
	sticky: true,
	captures_input: true,
	supports_window_mode: true,
	factory: || Box::new(TerminalBuffer::new()),
});

panel!(debug, {
	description: "Debug log viewer",
	mode_name: "DEBUG",
	layer: 2,
	sticky: true,
	factory: || Box::new(DebugPanel::new()),
});
