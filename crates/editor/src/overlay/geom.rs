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

		let border_type = match style.border_type {
			SurfaceBorder::Rounded => xeno_tui::widgets::BorderType::Rounded,
			SurfaceBorder::Stripe => xeno_tui::widgets::BorderType::Stripe,
		};
		let padding = xeno_tui::widgets::block::Padding::new(
			style.padding.left,
			style.padding.right,
			style.padding.top,
			style.padding.bottom,
		);

		let expected: Rect = xeno_tui::widgets::Block::default()
			.padding(padding)
			.borders(xeno_tui::widgets::Borders::ALL)
			.border_type(border_type)
			.title("Title")
			.inner(rect.into())
			.into();

		assert_eq!(pane_inner_rect(rect, &style), expected);
	}
}
