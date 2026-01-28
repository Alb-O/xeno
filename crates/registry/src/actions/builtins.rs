pub(crate) mod editing;
pub(crate) mod find;
pub(crate) mod insert;
pub(crate) mod misc;
pub(crate) mod modes;
pub(crate) mod navigation;
pub(crate) mod prefixes;
pub(crate) mod scrolling;
pub(crate) mod search;
pub(crate) mod selection;
pub(crate) mod text_objects;
pub(crate) mod window;

pub use navigation::{cursor_motion, selection_motion};

use crate::actions::ActionDef;
use crate::db::builder::{BuiltinGroup, RegistryDbBuilder};

const GROUPS: &[BuiltinGroup<ActionDef>] = &[
	BuiltinGroup::new("modes", modes::DEFS),
	BuiltinGroup::new("editing", editing::DEFS),
	BuiltinGroup::new("insert", insert::DEFS),
	BuiltinGroup::new("navigation", navigation::DEFS),
	BuiltinGroup::new("scrolling", scrolling::DEFS),
	BuiltinGroup::new("find", find::DEFS),
	BuiltinGroup::new("search", search::DEFS),
	BuiltinGroup::new("selection", selection::DEFS),
	BuiltinGroup::new("text_objects", text_objects::DEFS),
	BuiltinGroup::new("misc", misc::DEFS),
	BuiltinGroup::new("window", window::DEFS),
];

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	for group in GROUPS {
		builder.register_action_group(group);
	}
	prefixes::register_prefixes(builder);
}
