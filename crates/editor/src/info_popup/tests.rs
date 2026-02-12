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

#[test]
fn store_render_plan_carries_popup_fields() {
	let mut store = InfoPopupStore::default();
	let id = store.next_id();
	store.insert(InfoPopup {
		id,
		buffer_id: ViewId(42),
		anchor: PopupAnchor::Point { x: 7, y: 9 },
		content_width: 48,
		content_height: 12,
	});

	let plan = store.render_plan();
	assert_eq!(plan.len(), 1);
	let target = plan[0];
	assert_eq!(target.id, id);
	assert_eq!(target.buffer_id, ViewId(42));
	assert_eq!(target.content_width, 48);
	assert_eq!(target.content_height, 12);
	match target.anchor {
		InfoPopupRenderAnchor::Point { x, y } => {
			assert_eq!(x, 7);
			assert_eq!(y, 9);
		}
		InfoPopupRenderAnchor::Center => panic!("expected point anchor"),
	}
}

#[test]
fn store_render_plan_maps_window_anchor_to_center() {
	let mut store = InfoPopupStore::default();
	let id = store.next_id();
	store.insert(InfoPopup {
		id,
		buffer_id: ViewId(7),
		anchor: PopupAnchor::Window(WindowId(3)),
		content_width: 20,
		content_height: 5,
	});

	let plan = store.render_plan();
	assert_eq!(plan.len(), 1);
	assert!(matches!(plan[0].anchor, InfoPopupRenderAnchor::Center));
}

#[test]
fn store_render_plan_is_sorted_by_popup_id() {
	let mut store = InfoPopupStore::default();
	store.insert(InfoPopup {
		id: InfoPopupId(10),
		buffer_id: ViewId(1),
		anchor: PopupAnchor::Center,
		content_width: 10,
		content_height: 3,
	});
	store.insert(InfoPopup {
		id: InfoPopupId(2),
		buffer_id: ViewId(2),
		anchor: PopupAnchor::Center,
		content_width: 10,
		content_height: 3,
	});

	let plan = store.render_plan();
	assert_eq!(plan.len(), 2);
	assert_eq!(plan[0].id, InfoPopupId(2));
	assert_eq!(plan[1].id, InfoPopupId(10));
}
