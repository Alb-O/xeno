use tome_manifest::language;

language!(json, {
	extensions: &["json", "jsonc"],
	filenames: &[".prettierrc", ".eslintrc"],
	description: "JSON file",
});

language!(yaml, {
	extensions: &["yaml", "yml"],
	description: "YAML file",
});

language!(toml, {
	extensions: &["toml"],
	filenames: &["Cargo.toml", "Pipfile"],
	description: "TOML file",
});

language!(xml, {
	extensions: &["xml", "xsl", "xslt", "svg"],
	first_line_patterns: &["<?xml"],
	description: "XML file",
});
