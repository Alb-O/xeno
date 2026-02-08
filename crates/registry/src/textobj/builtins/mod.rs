//! Built-in text object implementations.

pub mod brackets;
pub mod quotes;
pub mod word;

pub fn register_builtins(builder: &mut crate::db::builder::RegistryDbBuilder) {
	let metadata = crate::kdl::loader::load_text_object_metadata();
	let handlers = inventory::iter::<crate::textobj::TextObjectHandlerReg>
		.into_iter()
		.map(|r| r.0);
	let linked = crate::kdl::link::link_text_objects(&metadata, handlers);

	for def in linked {
		builder.register_linked_text_object(def);
	}
}
