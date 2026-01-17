//! Runtime notification keys (result handlers, input state).

pub mod keys {

	// Viewport/screen motion
	notif!(
		viewport_unavailable,
		Error,
		"Viewport info unavailable for screen motion"
	);
	notif_alias!(
		viewport_height_unavailable,
		viewport_unavailable,
		"Viewport height unavailable for screen motion"
	);
	notif!(
		screen_motion_unavailable,
		Error,
		"Screen motion target is unavailable"
	);

	// Input state
	notif!(pending_prompt(prompt: &str), Info, prompt.to_string());
	notif!(count_display(count: usize), Info, count.to_string());

	// Debug
	notif!(
		unhandled_result(discriminant: impl core::fmt::Debug),
		Debug,
		format!("Unhandled action result: {:?}", discriminant)
	);
}
