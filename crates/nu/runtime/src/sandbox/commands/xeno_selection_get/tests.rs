use crate::sandbox::{create_engine_state, find_decl};

#[test]
fn xeno_selection_get_is_registered() {
	let engine_state = create_engine_state(None).expect("engine state");
	assert!(
		find_decl(&engine_state, "xeno selection get").is_some(),
		"xeno selection get command should be registered"
	);
}
