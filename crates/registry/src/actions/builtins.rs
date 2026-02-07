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

use crate::db::builder::RegistryDbBuilder;

pub fn register_builtins(builder: &mut RegistryDbBuilder) {
	let blob = crate::kdl::loader::load_action_metadata();
	let handlers = inventory::iter::<crate::actions::ActionHandlerReg>
		.into_iter()
		.map(|r| r.0);
	let linked = crate::kdl::link::link_actions(&blob, handlers);

	for def in linked {
		builder.register_linked_action(def);
	}

	let prefixes = crate::kdl::link::link_prefixes(&blob);
	for prefix in prefixes {
		builder.key_prefixes.push(prefix);
	}
}
