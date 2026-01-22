//! Runtime assets verification.
//!
//! This module provides tests to verify that embedded query files and themes
//! are correctly included in the binary.

#[cfg(test)]
mod tests {
	#[test]
	fn test_queries_embedded() {
		let languages: Vec<_> = xeno_runtime_data::queries::languages().collect();
		assert!(!languages.is_empty(), "Should have language directories");
		assert!(languages.contains(&"rust"), "Should have rust queries");

		let highlights = xeno_runtime_data::queries::get_str("rust", "highlights");
		assert!(highlights.is_some(), "Should have rust highlights.scm");
	}

	#[test]
	fn test_themes_embedded() {
		let themes: Vec<_> = xeno_runtime_data::themes::list().collect();
		assert!(!themes.is_empty(), "Should have theme files");
		assert!(themes.contains(&"gruvbox.kdl"), "Should have gruvbox.kdl");
		assert!(themes.contains(&"one_dark.kdl"), "Should have one_dark.kdl");
	}
}
