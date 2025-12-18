use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_JSON: FileTypeDef = FileTypeDef {
	name: "json",
	extensions: &["json", "jsonc"],
	filenames: &[".prettierrc", ".eslintrc"],
	first_line_patterns: &[],
	description: "JSON file",
};

#[distributed_slice(FILE_TYPES)]
static FT_YAML: FileTypeDef = FileTypeDef {
	name: "yaml",
	extensions: &["yaml", "yml"],
	filenames: &[],
	first_line_patterns: &[],
	description: "YAML file",
};

#[distributed_slice(FILE_TYPES)]
static FT_TOML: FileTypeDef = FileTypeDef {
	name: "toml",
	extensions: &["toml"],
	filenames: &["Cargo.toml", "Pipfile"],
	first_line_patterns: &[],
	description: "TOML file",
};

#[distributed_slice(FILE_TYPES)]
static FT_XML: FileTypeDef = FileTypeDef {
	name: "xml",
	extensions: &["xml", "xsl", "xslt", "svg"],
	filenames: &[],
	first_line_patterns: &["<?xml"],
	description: "XML file",
};
