use linkme::distributed_slice;

use crate::ext::{FileTypeDef, FILE_TYPES};

#[distributed_slice(FILE_TYPES)]
static FT_JAVA: FileTypeDef = FileTypeDef {
    name: "java",
    extensions: &["java"],
    filenames: &[],
    first_line_patterns: &[],
    description: "Java source file",
};
