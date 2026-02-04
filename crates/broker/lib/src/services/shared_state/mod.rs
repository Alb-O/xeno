//! Shared document state service with preferred-owner enforcement and idle unlocks.
//!
//! This actor manages the authoritative state of open documents within the broker.
//! It coordinates multiple editor sessions using a "Preferred Owner Writes" model,
//! where the focused editor is granted ownership and exclusive write access.
//! Ownership is automatically released on idle, blur, or disconnect.
//!
//! # Mental Model
//!
//! The service acts as the single source of truth for all participants. It
//! serializes all mutations, enforces ownership era (epoch) and edit sequence
//! (seq) constraints, and ensures durability via [`HistoryStore`].
//!
//! # Invariants
//!
//! - Preferred Owner Writes: Only the preferred owner (focused editor) may submit deltas.
//! - Atomic Transfer: Focus transitions MUST bump the epoch and reset the sequence.
//! - Authoritative LSP: LSP doc synchronization MUST be driven from this service's
//!   authoritative rope to ensure diagnostics are aligned with broker state.

mod commands;
mod handle;
mod service;

pub use commands::SharedStateCmd;
pub use handle::SharedStateHandle;
pub use service::SharedStateService;
