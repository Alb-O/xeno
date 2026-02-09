use xeno_primitives::Color;

use super::mod_types::THEMES;

/// Blend two colors with the given alpha (0.0 = bg, 1.0 = fg).
#[inline]
pub fn blend_colors(fg: Color, bg: Color, alpha: f32) -> Color {
	fg.blend(bg, alpha)
}

/// Suggest a similar theme name using fuzzy matching.
pub fn suggest_theme(name: &str) -> Option<String> {
	let name_lower = name.to_lowercase();
	let mut best_match = None;
	let mut best_score = 0.0;

	for theme in THEMES.snapshot_guard().iter_refs() {
		let theme_name = theme.name_str();
		let score = strsim::jaro_winkler(&name_lower, theme_name);
		if score > best_score {
			best_score = score;
			best_match = Some(theme_name.to_string());
		}

		for alias in theme.keys_resolved() {
			let score = strsim::jaro_winkler(&name_lower, alias);
			if score > best_score {
				best_score = score;
				best_match = Some(theme_name.to_string());
			}
		}
	}

	if best_score > 0.8 { best_match } else { None }
}
