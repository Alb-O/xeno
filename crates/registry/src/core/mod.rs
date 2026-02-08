//! Shared registry infrastructure.

pub mod capability;
pub mod def_input;
pub mod error;
pub mod index;
pub mod key;
pub mod meta;
pub mod plugin;
pub mod symbol;
pub mod traits;

pub use capability::{Capability, CapabilitySet};
pub use error::{CommandError, InsertAction, InsertFatal, RegistryError};
pub use index::{
	BuildEntry, Collision, CollisionKind, DuplicatePolicy, KeyKind, Party, RegistryBuilder,
	RegistryIndex, RegistryMetaRef, RegistryRef, Resolution, RuntimeEntry, RuntimeRegistry,
	Snapshot,
};
pub use key::{FromOptionValue, LookupKey, OptionDefault, OptionType, OptionValue};
pub use meta::{RegistryMeta, RegistryMetaStatic, RegistrySource, SymbolList};
pub use plugin::PluginDef;
pub use symbol::{
	ActionId, CommandId, DenseId, FrozenInterner, GutterId, HookId, Interner, InternerBuilder,
	MotionId, OptionId, OverlayId, StatuslineId, Symbol, TextObjectId, ThemeId,
};
pub use traits::{RegistryEntry, RegistryMetadata};
