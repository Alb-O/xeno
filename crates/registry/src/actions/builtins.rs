//! Built-in action implementations.

mod editing;
mod find;
mod insert;
mod misc;
mod modes;
mod navigation;
mod prefixes;
mod scrolling;
mod search;
mod selection;
mod text_objects;
mod window;

pub use editing::*;
pub use find::*;
pub use insert::*;
pub use misc::*;
pub use modes::*;
pub use navigation::*;
pub use scrolling::*;
pub use search::*;
pub use selection::*;
pub use text_objects::*;
pub use window::*;
pub use navigation::{cursor_motion, selection_motion};

use crate::actions::ActionDef;
use crate::db::builder::RegistryDbBuilder;

fn register_slice(builder: &mut RegistryDbBuilder, defs: &[&'static ActionDef]) {
	for def in defs {
		builder.register_action(def);
	}
}

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	register_slice(builder, modes::DEFS);
	register_slice(builder, editing::DEFS);
	register_slice(builder, insert::DEFS);
	register_slice(builder, navigation::DEFS);
	register_slice(builder, scrolling::DEFS);
	register_slice(builder, find::DEFS);
	register_slice(builder, search::DEFS);
	register_slice(builder, selection::DEFS);
	register_slice(builder, text_objects::DEFS);
	register_slice(builder, misc::DEFS);
	register_slice(builder, window::DEFS);
}
