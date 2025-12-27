use tome_tui::layout::{Position, Rect};
use tome_manifest::notifications::Anchor;

/// Calculate the anchor position within a frame area.
pub fn calculate_anchor_position(anchor: Anchor, frame_area: Rect) -> Position {
	match anchor {
		Anchor::TopLeft => Position::new(frame_area.x, frame_area.y),
		Anchor::TopCenter => Position::new(frame_area.x + frame_area.width / 2, frame_area.y),
		Anchor::TopRight => Position::new(frame_area.right().saturating_sub(1), frame_area.y),
		Anchor::MiddleLeft => Position::new(frame_area.x, frame_area.y + frame_area.height / 2),
		Anchor::MiddleCenter => Position::new(
			frame_area.x + frame_area.width / 2,
			frame_area.y + frame_area.height / 2,
		),
		Anchor::MiddleRight => Position::new(
			frame_area.right().saturating_sub(1),
			frame_area.y + frame_area.height / 2,
		),
		Anchor::BottomLeft => Position::new(frame_area.x, frame_area.bottom().saturating_sub(1)),
		Anchor::BottomCenter => Position::new(
			frame_area.x + frame_area.width / 2,
			frame_area.bottom().saturating_sub(1),
		),
		Anchor::BottomRight => Position::new(
			frame_area.right().saturating_sub(1),
			frame_area.bottom().saturating_sub(1),
		),
	}
}

/// Calculate the final rectangular area for a notification.
pub fn calculate_rect(
	anchor: Anchor,
	anchor_pos: Position,
	width: u16,
	height: u16,
	frame_area: Rect,
	exterior_padding: u16,
) -> Rect {
	let mut x = anchor_pos.x;
	let mut y = anchor_pos.y;

	match anchor {
		Anchor::TopCenter | Anchor::MiddleCenter | Anchor::BottomCenter => {
			x = x.saturating_sub(width / 2);
		}
		Anchor::TopRight | Anchor::MiddleRight | Anchor::BottomRight => {
			x = x.saturating_sub(width.saturating_sub(1));
		}
		_ => {}
	}

	match anchor {
		Anchor::MiddleLeft | Anchor::MiddleCenter | Anchor::MiddleRight => {
			y = y.saturating_sub(height / 2);
		}
		Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => {
			y = y.saturating_sub(height.saturating_sub(1));
		}
		_ => {}
	}

	match anchor {
		Anchor::TopLeft => {
			x = x.saturating_add(exterior_padding);
			y = y.saturating_add(exterior_padding);
		}
		Anchor::TopCenter => {
			y = y.saturating_add(exterior_padding);
		}
		Anchor::TopRight => {
			x = x.saturating_sub(exterior_padding);
			y = y.saturating_add(exterior_padding);
		}
		Anchor::MiddleLeft => {
			x = x.saturating_add(exterior_padding);
		}
		Anchor::MiddleCenter => {}
		Anchor::MiddleRight => {
			x = x.saturating_sub(exterior_padding);
		}
		Anchor::BottomLeft => {
			x = x.saturating_add(exterior_padding);
			y = y.saturating_sub(exterior_padding);
		}
		Anchor::BottomCenter => {
			y = y.saturating_sub(exterior_padding);
		}
		Anchor::BottomRight => {
			x = x.saturating_sub(exterior_padding);
			y = y.saturating_sub(exterior_padding);
		}
	}

	let clamped_width = width.min(frame_area.width);
	let clamped_height = height.min(frame_area.height);

	let final_x = x
		.max(frame_area.x)
		.min(frame_area.right().saturating_sub(clamped_width));

	let final_y = y
		.max(frame_area.y)
		.min(frame_area.bottom().saturating_sub(clamped_height));

	Rect::new(final_x, final_y, clamped_width, clamped_height)
}
