use super::*;

#[test]
fn test_load_embedded_themes() {
	let result = load_embedded_themes();
	assert!(!result.themes.is_empty());
	assert!(result.errors.is_empty());
	assert!(result.themes.iter().any(|t| t.name == "gruvbox"));
}
