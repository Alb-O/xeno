use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_C: FileTypeDef = FileTypeDef {
	name: "c",
	extensions: &["c", "h"],
	filenames: &[],
	first_line_patterns: &[],
	description: "C source file",
};

#[distributed_slice(FILE_TYPES)]
static FT_CPP: FileTypeDef = FileTypeDef {
	name: "cpp",
	extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx", "c++", "h++"],
	filenames: &[],
	first_line_patterns: &[],
	description: "C++ source file",
};
