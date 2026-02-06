//! Centralized registry index infrastructure.

mod build;
mod collision;
pub(crate) mod insert;
mod runtime;
mod types;

pub use build::RegistryBuilder;
pub use collision::{ChooseWinner, Collision, DuplicatePolicy, KeyKind, KeyStore};
pub use insert::{insert_id_key_runtime, insert_typed_key};
pub use runtime::{RegistryRef, RuntimeRegistry};
pub use types::{DefPtr, DefRef, RegistryIndex};

pub(crate) mod invariants;

#[cfg(test)]
mod tests;
