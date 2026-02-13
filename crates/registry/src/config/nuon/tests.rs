use super::*;

#[test]
fn parse_config_supports_options_languages_and_keys() {
	let input = r#"
{
options: {
	tab-width: 4,
	theme: "gruvbox",
},
languages: [
	{ name: "rust", options: { tab-width: 2, theme: "monokai" } },
],
keys: {
	normal: { "ctrl+s": "command:write" }
}
}
"#;

	let config = parse_config_str(input).expect("config should parse");

	assert_eq!(config.languages.len(), 1);
	assert_eq!(config.languages[0].name, "rust");
	assert_eq!(config.warnings.len(), 1);
	assert!(matches!(
		&config.warnings[0],
		ConfigWarning::ScopeMismatch {
			option,
			found_in: "language block",
			expected: "global options block"
		} if option == "theme"
	));

	let keys = config.keys.expect("keys should be parsed");
	assert_eq!(
		keys.modes.get("normal").and_then(|m| m.get("ctrl+s")).map(String::as_str),
		Some("command:write")
	);
}

#[test]
fn parse_config_rejects_unknown_top_level_field() {
	let input = r#"{ foo: 1 }"#;
	let err = parse_config_str(input).expect_err("unknown field should fail");
	assert!(matches!(err, ConfigError::UnknownField(field) if field == "config.foo"));
}

#[test]
fn parse_theme_standalone_supports_nuon() {
	let input = r##"
{
name: "nuon-demo",
variant: "dark",
palette: {
	base: "#101010",
	fg: "#f0f0f0",
},
ui: {
	bg: "$base",
	fg: "$fg",
	nontext-bg: "#0a0a0a",
	gutter-fg: "gray",
	cursor-bg: "white",
	cursor-fg: "black",
	cursorline-bg: "#202020",
	selection-bg: "blue",
	selection-fg: "white",
	message-fg: "yellow",
	command-input-fg: "white",
},
mode: {
	normal-bg: "blue",
	normal-fg: "white",
	insert-bg: "green",
	insert-fg: "black",
	prefix-bg: "magenta",
	prefix-fg: "white",
	command-bg: "yellow",
	command-fg: "black",
},
semantic: {
	error: "red",
	warning: "yellow",
	success: "green",
	info: "cyan",
	hint: "dark-gray",
	dim: "dark-gray",
	link: "cyan",
	match: "green",
	accent: "cyan",
},
popup: {
	bg: "#111111",
	fg: "white",
	border: "white",
	title: "yellow",
},
}
"##;

	let theme = parse_theme_standalone_str(input).expect("theme should parse");
	assert_eq!(theme.meta.name, "nuon-demo");
	assert_eq!(theme.meta.id, "xeno-registry::nuon-demo");
	assert!(matches!(theme.payload.variant, crate::themes::ThemeVariant::Dark));
}
