pub mod ext;
#[cfg(feature = "host")]
pub mod graphemes;
#[cfg(feature = "host")]
pub mod input;
#[cfg(feature = "host")]
pub mod key;
#[cfg(feature = "host")]
pub mod movement;
pub mod prelude;
#[cfg(feature = "host")]
pub mod range;
#[cfg(feature = "host")]
pub mod selection;
#[cfg(feature = "host")]
pub mod transaction;

#[cfg(feature = "host")]
pub use ext::{
	COMMANDS, CommandContext, CommandDef, CommandError, CommandResult, FILE_TYPES, FileTypeDef,
	MOTIONS, MotionDef, TEXT_OBJECTS, TextObjectDef,
};
#[cfg(feature = "host")]
pub use input::{InputHandler, KeyResult, Mode};
#[cfg(feature = "host")]
pub use key::{Key, KeyCode, Modifiers, MouseButton, MouseEvent, ScrollDirection, SpecialKey};
#[cfg(feature = "host")]
pub use movement::WordType;
#[cfg(feature = "host")]
pub use range::Range;
#[cfg(feature = "host")]
pub use ropey::{Rope, RopeSlice};
#[cfg(feature = "host")]
pub use selection::Selection;
#[cfg(feature = "host")]
pub use transaction::{ChangeSet, Transaction};
