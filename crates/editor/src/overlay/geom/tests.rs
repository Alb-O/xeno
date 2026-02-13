use super::pane_inner_rect;
use crate::geometry::Rect;
use crate::window::{SurfaceBorder, SurfacePadding, SurfaceStyle};

#[test]
fn pane_inner_rect_matches_block_inner() {
	let rect = Rect::new(10, 5, 30, 9);
	let style = SurfaceStyle {
		border: true,
		border_type: SurfaceBorder::Stripe,
		padding: SurfacePadding::horizontal(1),
		shadow: false,
		title: Some("Title".to_string()),
	};
	let expected = Rect::new(12, 6, 26, 7);
	assert_eq!(pane_inner_rect(rect, &style), expected);
}
