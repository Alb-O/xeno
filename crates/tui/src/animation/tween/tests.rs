use super::*;

#[test]
fn test_tween_immediate_value() {
	let tween = Tween::new(0.0f32, 100.0f32, Duration::from_millis(100));
	// Immediately after creation, value should be very close to start
	// (allowing for tiny elapsed time between creation and check)
	assert!(
		tween.value() < 1.0,
		"expected near-zero, got {}",
		tween.value()
	);
}

#[test]
fn test_tween_zero_duration() {
	let tween = Tween::new(0.0f32, 100.0f32, Duration::ZERO);
	assert!(tween.is_complete());
	assert_eq!(tween.value(), 100.0);
}

#[test]
fn test_tween_reversed() {
	let tween = Tween::new(0.0f32, 100.0f32, Duration::from_millis(100));
	let reversed = tween.reversed();
	assert_eq!(reversed.start, 100.0);
	assert_eq!(reversed.end, 0.0);
}

#[test]
fn test_toggle_initial_state() {
	let toggle = ToggleTween::new(0.0f32, 1.0f32, Duration::from_millis(100));
	assert!(!toggle.is_active());
	assert_eq!(toggle.value(), 0.0);
}

#[test]
fn test_toggle_activation() {
	let mut toggle = ToggleTween::new(0.0f32, 1.0f32, Duration::from_millis(100));
	let changed = toggle.set_active(true);
	assert!(changed);
	assert!(toggle.is_active());

	// Setting to same state should return false
	let changed = toggle.set_active(true);
	assert!(!changed);
}
