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
	normal: { "ctrl+s": { kind: "command", name: "write" } }
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
	let binding = keys.modes.get("normal").and_then(|m| m.get("ctrl+s")).expect("binding should exist");
	assert!(matches!(binding, crate::invocation::Invocation::Command { name, .. } if name == "write"));
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

#[test]
fn parse_config_nu_decode_limits() {
	let input = r#"{
		nu: {
			decode: {
				macro: { max_invocations: 512, max_nodes: 100000 },
				hook: { max_invocations: 16 }
			}
		}
	}"#;
	let config = parse_config_str(input).expect("nu decode config should parse");
	let nu = config.nu.expect("nu config should be present");
	let macro_limits = nu.decode_macro.expect("macro limits should be present");
	assert_eq!(macro_limits.max_invocations, Some(512));
	assert_eq!(macro_limits.max_nodes, Some(100000));

	let hook_limits = nu.decode_hook.expect("hook limits should be present");
	assert_eq!(hook_limits.max_invocations, Some(16));
	assert_eq!(hook_limits.max_nodes, None);
}

#[test]
fn parse_config_nu_decode_limits_apply() {
	let overrides = super::super::DecodeLimitOverrides {
		max_invocations: Some(10),
		..Default::default()
	};
	let base = xeno_invocation::nu::DecodeLimits::macro_defaults();
	let applied = overrides.apply(base);
	assert_eq!(applied.max_invocations, 10);
	assert_eq!(applied.max_nodes, base.max_nodes);
}

#[test]
fn parse_config_nu_rejects_unknown_decode_field() {
	let input = r#"{ nu: { decode: { macro: { bogus: 1 } } } }"#;
	let err = parse_config_str(input).expect_err("unknown field should fail");
	assert!(matches!(err, super::super::ConfigError::UnknownField(f) if f.contains("bogus")));
}

#[test]
fn parse_keys_string_spec_command() {
	let input = r#"{ keys: { normal: { "ctrl+s": "command:write" } } }"#;
	let config = parse_config_str(input).expect("string spec should parse");
	let keys = config.keys.expect("keys should be present");
	let bindings = keys.modes.get("normal").expect("normal mode should exist");
	let inv = bindings.get("ctrl+s").expect("ctrl+s should be bound");
	assert!(matches!(inv, xeno_invocation::Invocation::Command { name, args } if name == "write" && args.is_empty()));
}

#[test]
fn parse_keys_string_spec_with_quoted_args() {
	let input = r#"{ keys: { normal: { "ctrl+o": "command:open \"my file.txt\"" } } }"#;
	let config = parse_config_str(input).expect("quoted args should parse");
	let keys = config.keys.expect("keys should be present");
	let bindings = keys.modes.get("normal").expect("normal mode should exist");
	let inv = bindings.get("ctrl+o").expect("ctrl+o should be bound");
	assert!(matches!(inv, xeno_invocation::Invocation::Command { name, args } if name == "open" && args == &["my file.txt"]));
}

#[test]
fn parse_keys_invalid_string_spec_errors() {
	let input = r#"{ keys: { normal: { "ctrl+x": "bogus:nope" } } }"#;
	let err = parse_config_str(input).expect_err("invalid spec should fail");
	assert!(matches!(err, super::super::ConfigError::InvalidKeyBinding(msg) if msg.contains("keys.normal.ctrl+x")));
}
