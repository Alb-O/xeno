use crate::sandbox::{create_engine_state, find_decl};

#[test]
fn xeno_buffer_get_is_registered() {
	let engine_state = create_engine_state(None).expect("engine state");
	assert!(
		find_decl(&engine_state, "xeno buffer get").is_some(),
		"xeno buffer get command should be registered"
	);
}
