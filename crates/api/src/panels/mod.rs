//! Panel definitions and runtime management.
//!
//! This module defines the built-in panels (terminal, debug) using the [`panel!`] macro
//! and registers factories separately, and provides [`PanelRegistry`] for runtime instance management.

mod registry;

use evildoer_registry::panels::keys as panel_ids;
use evildoer_registry::{panel, register_panel_factory};
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
});

panel!(debug, {
	description: "Debug log viewer",
	mode_name: "DEBUG",
	layer: 2,
	sticky: true,
});

register_panel_factory!(terminal, panel_ids::terminal, || Box::new(
	TerminalBuffer::new()
));
register_panel_factory!(debug, panel_ids::debug, || Box::new(DebugPanel::new()));
