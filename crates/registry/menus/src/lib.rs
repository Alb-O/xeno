//! Menu bar registry: groups, items, and registration macros.

mod def;
/// Built-in menu definitions.
mod impls;
#[doc(hidden)]
mod macros;

pub use def::*;
pub use evildoer_registry_core::{RegistryMetadata, RegistrySource, impl_registry_metadata};
