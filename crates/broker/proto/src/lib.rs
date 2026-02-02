//! Shared wire types for xeno-broker IPC.
//!
//! This crate defines the protocol messages exchanged between the editor and the broker
//! over Unix domain sockets. The protocol uses binary framing with postcard encoding
//! for efficiency.

#![warn(missing_docs)]

pub mod paths;
pub mod protocol;
pub mod types;

pub use protocol::BrokerProtocol;
pub use types::*;
