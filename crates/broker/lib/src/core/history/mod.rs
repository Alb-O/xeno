//! Broker-owned history store for shared documents.
//!
//! Implements a branching history graph backed by `helix-db`. This store manages
//! the persistence of document states, including snapshots and deltas, enabling
//! authoritative undo/redo coordination across multiple editor sessions.
//!
//! # Mental Model
//!
//! The history is modeled as a set of linear chains rooted at checkpoints.
//! Periodic compaction squashes older deltas into new root snapshots to bound
//! traversal costs and storage growth.
//!
//! # Invariants
//!
//! - Linear Ancestry: Every non-root node MUST have exactly one parent.
//! - Single Root: A document history graph MUST have exactly one node marked as root.
//! - Authoritative Fingerprint: Every node stores the `hash64` and `len_chars`
//!   representing the state after applying its transaction.

mod internals;
mod store;
mod types;

pub use store::HistoryStore;
pub use types::{HistoryError, HistoryMeta, StoredDoc};
