use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_TYPESCRIPT: FileTypeDef = FileTypeDef {
	name: "typescript",
	extensions: &["ts", "mts", "cts"],
	filenames: &[],
	first_line_patterns: &[],
	description: "TypeScript source file",
};

#[distributed_slice(FILE_TYPES)]
static FT_TSX: FileTypeDef = FileTypeDef {
	name: "tsx",
	extensions: &["tsx"],
	filenames: &[],
	first_line_patterns: &[],
	description: "TypeScript JSX file",
};
