//! Built-in ACP (Agent Client Protocol) integration.
//!
//! This module provides AI-assisted editing capabilities through the Agent Client Protocol.
//! It manages communication with an external AI agent (opencode) and provides commands
//! for starting/stopping the agent, toggling the chat panel, and inserting responses.

mod backend;
mod commands;
mod handler;
mod state;

pub use state::{AcpEvent, AcpManager, ChatRole};
