//! Centralized registry index infrastructure.

mod build;
mod collision;
pub(crate) mod runtime;
mod types;

pub use build::{BuildEntry, RegistryBuilder, RegistryMetaRef};
pub use collision::{Collision, CollisionKind, DuplicatePolicy, KeyKind, Party, Resolution};
pub use runtime::{RegistryRef, RuntimeEntry, RuntimeRegistry, Snapshot};
pub use types::RegistryIndex;

#[cfg(any(test, doc))]
pub(crate) mod invariants;

#[cfg(test)]
pub(crate) mod test_fixtures;

#[cfg(test)]
mod tests;
