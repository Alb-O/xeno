use super::*;

#[test]
fn test_parse_standalone_theme() {
	let kdl = xeno_runtime_data::themes::get_str("gruvbox.kdl").unwrap();
	let theme = parse_standalone_theme(kdl).unwrap();

	assert_eq!(theme.name, "gruvbox");
	assert_eq!(theme.variant, ThemeVariant::Dark);
	assert_eq!(theme.aliases, vec!["gruvbox_dark", "gruvbox-dark"]);

	// Verify syntax styles are parsed
	let keyword_style = theme.colors.syntax.resolve("keyword");
	assert!(
		keyword_style.fg.is_some(),
		"keyword style should have fg color"
	);

	let comment_style = theme.colors.syntax.resolve("comment");
	assert!(
		comment_style.fg.is_some(),
		"comment style should have fg color"
	);
}
