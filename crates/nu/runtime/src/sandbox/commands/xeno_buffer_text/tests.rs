use crate::sandbox::{create_engine_state, find_decl};

#[test]
fn xeno_buffer_text_is_registered() {
	let engine_state = create_engine_state(None).expect("engine state");
	assert!(
		find_decl(&engine_state, "xeno buffer text").is_some(),
		"xeno buffer text command should be registered"
	);
}
