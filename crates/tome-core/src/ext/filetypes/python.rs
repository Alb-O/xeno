use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_PYTHON: FileTypeDef = FileTypeDef {
	name: "python",
	extensions: &["py", "pyw", "pyi"],
	filenames: &[],
	first_line_patterns: &["python", "python3"],
	description: "Python source file",
};
