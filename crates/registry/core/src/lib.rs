//! Shared registry infrastructure.

mod capability;
mod error;
mod index;
mod key;
mod meta;
mod traits;

pub use capability::Capability;
pub use error::CommandError;
pub use index::{
	ChooseWinner, Collision, DuplicatePolicy, InsertAction, InsertFatal, KeyKind, KeyStore,
	RegistryBuilder, RegistryIndex, RegistryReg, RuntimeRegistry, build_map, insert_typed_key,
};
pub use key::{FromOptionValue, Key, OptionType, OptionValue};
pub use meta::{ActionId, RegistryMeta, RegistrySource};
pub use traits::{RegistryEntry, RegistryMetadata};
