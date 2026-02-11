//! Runtime assets verification.
//!
//! This module provides tests to verify registry-backed language assets load.

#[cfg(test)]
mod tests {
	#[test]
	fn test_queries_from_registry() {
		let rust = xeno_registry::LANGUAGES.get("rust").expect("rust language should exist");
		let highlights = xeno_registry::languages::queries::get_query_text(&rust, "highlights");
		assert!(highlights.is_some(), "Should have rust highlights.scm");
	}

	#[test]
	fn test_themes_from_registry() {
		assert!(xeno_registry::THEMES.get("gruvbox").is_some());
		assert!(xeno_registry::THEMES.get("one_dark").is_some());
	}
}
