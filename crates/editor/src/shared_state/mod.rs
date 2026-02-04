//! Shared document state manager for broker-backed synchronization.
//!
//! The [`SharedStateManager`] tracks the synchronization lifecycle for every
//! shared document open in the editor. It manages ownership transitions,
//! edit sequencing, and ensures local state remains aligned with the broker's
//! authoritative truth using nonces and fingerprints.

mod apply;
pub mod convert;
mod focus;
mod lifecycle;
mod manager;
mod resync;
mod sync;
mod types;

#[cfg(test)]
mod tests;

pub use manager::SharedStateManager;
pub use types::{
	GroupViewState, ResyncRequest, SharedStateEvent, SharedStateRole, SharedViewHistory, SyncStatus,
};
