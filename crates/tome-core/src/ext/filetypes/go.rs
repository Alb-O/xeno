use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_GO: FileTypeDef = FileTypeDef {
	name: "go",
	extensions: &["go"],
	filenames: &[],
	first_line_patterns: &[],
	description: "Go source file",
};
