use crate::geometry::Rect;
use crate::window::SurfaceStyle;

pub fn pane_inner_rect(rect: Rect, style: &SurfaceStyle) -> Rect {
	let border_left = u16::from(style.border);
	let border_right = u16::from(style.border);
	let border_top = u16::from(style.border);
	let border_bottom = u16::from(style.border);

	let x = rect.x.saturating_add(border_left).saturating_add(style.padding.left);
	let y = rect.y.saturating_add(border_top).saturating_add(style.padding.top);
	let horizontal = border_left
		.saturating_add(border_right)
		.saturating_add(style.padding.left)
		.saturating_add(style.padding.right);
	let vertical = border_top
		.saturating_add(border_bottom)
		.saturating_add(style.padding.top)
		.saturating_add(style.padding.bottom);

	Rect::new(
		x,
		y,
		rect.width.saturating_sub(horizontal),
		rect.height.saturating_sub(vertical),
	)
}

#[cfg(test)]
mod tests {
	use xeno_tui::widgets::BorderType;
	use xeno_tui::widgets::block::Padding;

	use super::pane_inner_rect;
	use crate::geometry::Rect;
	use crate::window::SurfaceStyle;

	#[test]
	fn pane_inner_rect_matches_block_inner() {
		let rect = Rect::new(10, 5, 30, 9);
		let style = SurfaceStyle {
			border: true,
			border_type: BorderType::Stripe,
			padding: Padding::horizontal(1),
			shadow: false,
			title: Some("Title".to_string()),
		};

		let expected: Rect = xeno_tui::widgets::Block::default()
			.padding(style.padding)
			.borders(xeno_tui::widgets::Borders::ALL)
			.border_type(style.border_type)
			.title("Title")
			.inner(rect.into())
			.into();

		assert_eq!(pane_inner_rect(rect, &style), expected);
	}
}
