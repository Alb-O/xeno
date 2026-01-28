//! Shared registry infrastructure.

pub mod capability;
pub mod error;
pub mod index;
pub mod key;
pub mod meta;
pub mod plugin;
pub mod runtime_alloc;
pub mod traits;

pub use capability::Capability;
pub use error::{CommandError, InsertAction, InsertFatal, RegistryError};
pub use index::{
	ChooseWinner, Collision, DuplicatePolicy, KeyKind, KeyStore, RegistryBuilder, RegistryIndex,
	RuntimeRegistry, insert_typed_key,
};
pub use key::{FromOptionValue, Key, OptionDefault, OptionType, OptionValue};
pub use meta::{ActionId, RegistryMeta, RegistrySource};
pub use plugin::PluginDef;
pub use traits::{RegistryEntry, RegistryMetadata};
