use std::collections::HashSet;

use crate::kdl::loader::load_motion_metadata;

#[test]
fn all_kdl_motions_have_handlers() {
	use crate::motions::handler::MotionHandlerStatic;
	let blob = load_motion_metadata();
	let handlers: Vec<&MotionHandlerStatic> = inventory::iter::<crate::motions::MotionHandlerReg>
		.into_iter()
		.map(|r| r.0)
		.collect();
	let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();

	for motion in &blob.motions {
		assert!(
			handler_names.contains(motion.common.name.as_str()),
			"KDL motion '{}' has no handler",
			motion.common.name
		);
	}
}

#[test]
fn all_motion_handlers_have_kdl_entries() {
	let blob = load_motion_metadata();
	let kdl_names: HashSet<&str> = blob
		.motions
		.iter()
		.map(|m| m.common.name.as_str())
		.collect();

	for reg in inventory::iter::<crate::motions::MotionHandlerReg> {
		assert!(
			kdl_names.contains(reg.0.name),
			"motion_handler!({}) has no KDL entry",
			reg.0.name
		);
	}
}
