//! Built-in ACP (Agent Client Protocol) integration.
//!
//! This module provides AI-assisted editing capabilities through the Agent Client Protocol.
//! It manages communication with an external AI agent (opencode) and provides commands
//! for starting/stopping the agent, toggling the chat panel, and inserting responses.
//!
//! ## Module Structure
//!
//! - `types`: Core types shared across the application (ChatRole, ChatItem, AcpEvent, etc.)
//! - `state`: The AcpManager that orchestrates the backend
//! - `backend`: Async communication with the ACP agent
//! - `handler`: Protocol message handling
//! - `commands`: Ex-mode commands (registered via distributed_slice)

mod backend;
mod commands;
mod handler;
pub mod panel;
mod state;
pub mod types;

use linkme::distributed_slice;

use crate::editor::extensions::{EXTENSIONS, ExtensionInitDef, ExtensionTickDef, TICK_EXTENSIONS};

#[distributed_slice(EXTENSIONS)]
static ACP_INIT: ExtensionInitDef = ExtensionInitDef {
	id: "acp",
	priority: 100,
	init: |map| {
		map.insert(AcpManager::new());
	},
};

#[distributed_slice(TICK_EXTENSIONS)]
static ACP_TICK: ExtensionTickDef = ExtensionTickDef {
	priority: 100,
	tick: |editor| {
		panel::poll_acp_events(editor);
	},
};

// Re-export the manager and commonly used types at the module root
pub use state::AcpManager;
pub use types::{AcpEvent, ChatItem, ChatPanelState, ChatRole};
