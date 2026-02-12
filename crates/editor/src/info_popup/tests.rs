use super::*;

#[test]
fn measure_content_clamps_width_and_height() {
	let long_line = "x".repeat(80);
	let content = (0..30).map(|_| long_line.as_str()).collect::<Vec<_>>().join("\n");
	let (w, h) = measure_content(&content);
	assert_eq!(w, 60);
	assert_eq!(h, 20);
}

#[test]
fn store_next_id_is_monotonic() {
	let mut store = InfoPopupStore::default();
	assert_eq!(store.next_id().0, 0);
	assert_eq!(store.next_id().0, 1);
	assert_eq!(store.next_id().0, 2);
}
