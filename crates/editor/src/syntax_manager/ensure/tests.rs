use super::*;

#[test]
fn test_compute_viewport_key_aligns_to_half_window_stride() {
	let key = compute_viewport_key(70_000, 131_072);
	assert_eq!(key, ViewportKey(65_536));
}

#[test]
fn test_compute_viewport_key_respects_min_stride_floor() {
	let key = compute_viewport_key(9_000, 4_096);
	assert_eq!(key, ViewportKey(8_192));
}
