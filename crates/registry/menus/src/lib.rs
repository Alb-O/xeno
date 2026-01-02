//! Menu bar registry: groups, items, and registration macros.

mod def;
/// Built-in menu definitions.
mod impls;
#[doc(hidden)]
mod macros;

pub use def::*;
pub use evildoer_registry_motions::{impl_registry_metadata, RegistryMetadata, RegistrySource};
