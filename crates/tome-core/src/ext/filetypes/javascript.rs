use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_JAVASCRIPT: FileTypeDef = FileTypeDef {
	name: "javascript",
	extensions: &["js", "mjs", "cjs"],
	filenames: &[],
	first_line_patterns: &["node"],
	description: "JavaScript source file",
};

#[distributed_slice(FILE_TYPES)]
static FT_JSX: FileTypeDef = FileTypeDef {
	name: "jsx",
	extensions: &["jsx"],
	filenames: &[],
	first_line_patterns: &[],
	description: "JavaScript JSX file",
};
