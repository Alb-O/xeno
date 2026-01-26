//! Action notification keys.

pub mod keys {
	notif!(unknown_action(name: &str), Error, format!("Unknown action: {}", name));
	notif!(action_error(err: impl core::fmt::Display), Error, err.to_string());
}
