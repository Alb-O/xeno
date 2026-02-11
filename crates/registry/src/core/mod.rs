//! Shared registry infrastructure.

pub mod capability;
pub mod def_input;
pub mod error;
pub mod handler_static;
pub mod index;
pub mod key;
pub mod linked_def;
pub mod meta;
pub mod plugin;
pub mod symbol;
pub mod traits;

pub use capability::{Capability, CapabilitySet};
pub use error::{CommandError, InsertAction, InsertFatal, RegistryError};
pub use handler_static::HandlerStatic;
pub use index::{
	BuildEntry, Collision, CollisionKind, DuplicatePolicy, KeyKind, Party, RegistryBuilder, RegistryIndex, RegistryMetaRef, RegistryRef, Resolution,
	RuntimeEntry, RuntimeRegistry, Snapshot, StrListRef,
};
pub use key::{FromOptionValue, LookupKey, OptionDefault, OptionType, OptionValue};
pub use linked_def::{LinkedDef, LinkedMetaOwned, LinkedPayload};
pub use meta::{RegistryMeta, RegistryMetaStatic, RegistrySource, SymbolList};
pub use plugin::PluginDef;
pub use symbol::{
	ActionId, CommandId, DenseId, FrozenInterner, GutterId, HookId, Interner, InternerBuilder, LanguageId, MotionId, NotificationId, OptionId, OverlayId,
	StatuslineId, Symbol, TextObjectId, ThemeId,
};
pub use traits::{RegistryEntry, RegistryMetadata};
