use super::super::*;
use crate::kdl::loader::load_text_object_metadata;

#[test]
fn all_kdl_text_objects_have_handlers() {
	use crate::textobj::handler::TextObjectHandlerStatic;
	let blob = load_text_object_metadata();
	let handlers: Vec<&TextObjectHandlerStatic> =
		inventory::iter::<crate::textobj::TextObjectHandlerReg>
			.into_iter()
			.map(|r| r.0)
			.collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();

	for obj in &blob.text_objects {
		assert!(
			handler_names.contains(obj.name.as_str()),
			"KDL text object '{}' has no handler",
			obj.name
		);
	}
}

#[test]
fn all_text_object_handlers_have_kdl_entries() {
	let blob = load_text_object_metadata();
	let kdl_names: HashSet<&str> = blob.text_objects.iter().map(|t| t.name.as_str()).collect();

	for reg in inventory::iter::<crate::textobj::TextObjectHandlerReg> {
		assert!(
			kdl_names.contains(reg.0.name),
			"text_object_handler!({}) has no KDL entry",
			reg.0.name
		);
	}
}
