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
mod state;
pub mod types;

// Re-export the manager and commonly used types at the module root
pub use state::AcpManager;
pub use types::{AcpEvent, ChatItem, ChatPanelState, ChatRole};
