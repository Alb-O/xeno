use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_MARKDOWN: FileTypeDef = FileTypeDef {
	name: "markdown",
	extensions: &["md", "markdown", "mkd", "mkdn"],
	filenames: &["README", "CHANGELOG", "CONTRIBUTING", "LICENSE"],
	first_line_patterns: &[],
	description: "Markdown file",
};

#[distributed_slice(FILE_TYPES)]
static FT_RST: FileTypeDef = FileTypeDef {
	name: "rst",
	extensions: &["rst", "rest"],
	filenames: &[],
	first_line_patterns: &[],
	description: "reStructuredText file",
};

#[distributed_slice(FILE_TYPES)]
static FT_ASCIIDOC: FileTypeDef = FileTypeDef {
	name: "asciidoc",
	extensions: &["adoc", "asciidoc", "asc"],
	filenames: &[],
	first_line_patterns: &[],
	description: "AsciiDoc file",
};

#[distributed_slice(FILE_TYPES)]
static FT_TEX: FileTypeDef = FileTypeDef {
	name: "tex",
	extensions: &["tex", "sty", "cls"],
	filenames: &[],
	first_line_patterns: &[],
	description: "LaTeX file",
};
