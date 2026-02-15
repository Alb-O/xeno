//! Generic async RPC message pump and protocol framework.
//!
//! This crate provides protocol-agnostic primitives for building async RPC systems:
//! * `MainLoop`: A generic tokio-driven message pump
//! * `PeerSocket`: Internal channel for communication with the main loop
//! * `Protocol`: Trait for defining wire formats and message semantics
//! * `AnyEvent`: Type-erased loopback event container

#![warn(missing_docs)]

pub mod error;
pub mod event;
pub mod mainloop;
pub mod protocol;
pub mod socket;

pub use error::{Error, Result};
pub use event::AnyEvent;
pub use mainloop::{MainLoop, RpcService};
pub use protocol::{CounterIdGen, Inbound, Protocol};
pub use socket::{MainLoopEvent, PeerSocket};
