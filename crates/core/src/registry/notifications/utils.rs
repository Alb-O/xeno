use ratatui::style::Color;

/// Converts a ratatui Color to an RGB tuple.
#[inline]
pub fn color_to_rgb(color: Option<Color>) -> Option<(u8, u8, u8)> {
	match color {
		Some(Color::Black) => Some((0, 0, 0)),
		Some(Color::Red) => Some((255, 0, 0)),
		Some(Color::Green) => Some((0, 255, 0)),
		Some(Color::Yellow) => Some((255, 255, 0)),
		Some(Color::Blue) => Some((0, 0, 255)),
		Some(Color::Magenta) => Some((255, 0, 255)),
		Some(Color::Cyan) => Some((0, 255, 255)),
		Some(Color::Gray) => Some((128, 128, 128)),
		Some(Color::DarkGray) => Some((64, 64, 64)),
		Some(Color::LightRed) => Some((255, 128, 128)),
		Some(Color::LightGreen) => Some((128, 255, 128)),
		Some(Color::LightYellow) => Some((255, 255, 128)),
		Some(Color::LightBlue) => Some((128, 128, 255)),
		Some(Color::LightMagenta) => Some((255, 128, 255)),
		Some(Color::LightCyan) => Some((128, 255, 255)),
		Some(Color::White) => Some((255, 255, 255)),
		Some(Color::Rgb(r, g, b)) => Some((r, g, b)),
		_ => None,
	}
}

/// Applies quadratic ease-in easing.
#[inline]
pub fn ease_in_quad(t: f32) -> f32 {
	t * t
}

/// Applies quadratic ease-out easing.
#[inline]
pub fn ease_out_quad(t: f32) -> f32 {
	t * (2.0 - t)
}

/// Performs linear interpolation between two values.
#[inline]
pub fn lerp(start: f32, end: f32, t: f32) -> f32 {
	start + t * (end - start)
}
