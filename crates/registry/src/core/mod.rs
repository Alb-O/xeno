//! Shared registry infrastructure.

pub mod def_input;
pub mod error;
pub mod handler_static;
pub mod index;
pub mod key;
pub mod linked_def;
pub mod meta;
pub mod symbol;
pub mod traits;

pub use error::{CommandError, InsertAction, InsertFatal, RegistryError};
pub use handler_static::HandlerStatic;
pub use index::{
	BuildEntry, Collision, CollisionKind, DuplicatePolicy, KeyKind, Party, RegistryBuilder, RegistryIndex, RegistryMetaRef, RegistryRef, Resolution,
	RuntimeEntry, RuntimeRegistry, Snapshot, StrListRef,
};
pub use key::{FromOptionValue, LookupKey, OptionDefault, OptionType, OptionValue};
pub use linked_def::{LinkedDef, LinkedMetaOwned, LinkedPayload};
pub use meta::{RegistryMeta, RegistryMetaStatic, RegistrySource, SymbolList};
pub use symbol::{
	ActionId, CommandId, DenseId, FrozenInterner, GutterId, HookId, Interner, InternerBuilder, LanguageId, MotionId, NotificationId, OptionId, OverlayId,
	SnippetId, StatuslineId, Symbol, TextObjectId, ThemeId,
};
pub use traits::{RegistryEntry, RegistryMetadata};
