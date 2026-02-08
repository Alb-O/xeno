use super::super::*;
use crate::kdl::loader::load_theme_metadata;

#[test]
fn all_kdl_themes_parse_and_validate() {
	let blob = load_theme_metadata();
	// This will panic if any theme has invalid colors, unresolved palette refs, or bad modifiers
	link_themes(&blob);
}

#[test]
fn default_theme_exists_in_kdl() {
	let blob = load_theme_metadata();
	let names: HashSet<&str> = blob.themes.iter().map(|t| t.name.as_str()).collect();
	assert!(
		names.contains("monokai"),
		"Default theme 'monokai' missing from KDL"
	);
}

#[test]
fn modifier_parsing_works() {
	use crate::themes::Modifier;
	assert_eq!(
		themes::parse_modifiers("bold", "test", "test"),
		Modifier::BOLD
	);
	assert_eq!(
		themes::parse_modifiers("bold|italic", "test", "test"),
		Modifier::BOLD | Modifier::ITALIC
	);
	assert_eq!(
		themes::parse_modifiers("  bold | ITALIC  ", "test", "test"),
		Modifier::BOLD | Modifier::ITALIC
	);
}

#[test]
#[should_panic(expected = "Theme 'test' scope 'test' unknown modifier: 'invalid'")]
fn modifier_parsing_panics_on_unknown() {
	themes::parse_modifiers("invalid", "test", "test");
}
