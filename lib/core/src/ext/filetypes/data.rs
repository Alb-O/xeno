use crate::filetype;

filetype!(json, {
	extensions: &["json", "jsonc"],
	filenames: &[".prettierrc", ".eslintrc"],
	description: "JSON file",
});

filetype!(yaml, {
	extensions: &["yaml", "yml"],
	description: "YAML file",
});

filetype!(toml, {
	extensions: &["toml"],
	filenames: &["Cargo.toml", "Pipfile"],
	description: "TOML file",
});

filetype!(xml, {
	extensions: &["xml", "xsl", "xslt", "svg"],
	first_line_patterns: &["<?xml"],
	description: "XML file",
});
