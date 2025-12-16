use linkme::distributed_slice;

use crate::ext::{FileTypeDef, FILE_TYPES};

#[distributed_slice(FILE_TYPES)]
static FT_RUST: FileTypeDef = FileTypeDef {
    name: "rust",
    extensions: &["rs"],
    filenames: &[],
    first_line_patterns: &[],
    description: "Rust source file",
};
