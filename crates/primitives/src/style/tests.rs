use super::Color;

#[test]
fn blend_alpha_preserves_existing_semantics() {
	let fg = Color::Rgb(200, 0, 0);
	let bg = Color::Rgb(0, 0, 200);
	assert_eq!(fg.blend(bg, 0.0), Color::Rgb(0, 0, 200));
	assert_eq!(fg.blend(bg, 1.0), Color::Rgb(200, 0, 0));
}

#[test]
fn contrast_ratio_black_white_is_wcag_max() {
	let ratio = Color::Black.contrast_ratio(Color::White);
	assert!((ratio - 21.0).abs() < 0.1, "ratio={ratio}");
}

#[test]
fn ensure_min_contrast_is_noop_when_already_sufficient() {
	let fg = Color::Rgb(180, 180, 180);
	let bg = Color::Rgb(20, 20, 20);
	let adjusted = fg.ensure_min_contrast(bg, 1.5);
	assert_eq!(adjusted.to_rgb(), fg.to_rgb());
}

#[test]
fn ensure_min_contrast_boosts_when_needed() {
	let fg = Color::Rgb(40, 40, 40);
	let bg = Color::Rgb(30, 30, 30);
	let adjusted = fg.ensure_min_contrast(bg, 1.5);
	assert!(adjusted.contrast_ratio(bg) >= 1.5);
}
