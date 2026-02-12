use crate::geometry::Rect;
use xeno_tui::widgets::{Block, Borders};

use crate::window::SurfaceStyle;

pub fn pane_inner_rect(rect: Rect, style: &SurfaceStyle) -> Rect {
	let mut block = Block::default().padding(style.padding);
	if style.border {
		block = block.borders(Borders::ALL).border_type(style.border_type);
		if let Some(title) = &style.title {
			block = block.title(title.as_str());
		}
	}
	block.inner(rect)
}

#[cfg(test)]
mod tests {
	use crate::geometry::Rect;
	use xeno_tui::widgets::BorderType;
	use xeno_tui::widgets::block::Padding;

	use super::pane_inner_rect;
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

		let expected = xeno_tui::widgets::Block::default()
			.padding(style.padding)
			.borders(xeno_tui::widgets::Borders::ALL)
			.border_type(style.border_type)
			.title("Title")
			.inner(rect);

		assert_eq!(pane_inner_rect(rect, &style), expected);
	}
}
