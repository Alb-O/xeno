use xeno_primitives::Color;

use super::mod_types::THEMES;

/// Blend two colors with the given alpha (0.0 = bg, 1.0 = fg).
#[inline]
pub fn blend_colors(fg: Color, bg: Color, alpha: f32) -> Color {
	fg.blend(bg, alpha)
}

/// Suggest a similar theme name using fuzzy matching.
pub fn suggest_theme(name: &str) -> Option<&'static str> {
	let name = name.to_lowercase();
	let mut best_match = None;
	let mut best_score = 0.0;

	for theme in THEMES.iter() {
		let score = strsim::jaro_winkler(&name, theme.meta.name);
		if score > best_score {
			best_score = score;
			best_match = Some(theme.meta.name);
		}

		for alias in theme.meta.aliases {
			let score = strsim::jaro_winkler(&name, alias);
			if score > best_score {
				best_score = score;
				best_match = Some(theme.meta.name);
			}
		}
	}

	if best_score > 0.8 { best_match } else { None }
}
