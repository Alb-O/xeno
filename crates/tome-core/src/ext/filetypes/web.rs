use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_HTML: FileTypeDef = FileTypeDef {
	name: "html",
	extensions: &["html", "htm", "xhtml"],
	filenames: &[],
	first_line_patterns: &["<!DOCTYPE html", "<!doctype html"],
	description: "HTML file",
};

#[distributed_slice(FILE_TYPES)]
static FT_CSS: FileTypeDef = FileTypeDef {
	name: "css",
	extensions: &["css"],
	filenames: &[],
	first_line_patterns: &[],
	description: "CSS file",
};

#[distributed_slice(FILE_TYPES)]
static FT_SCSS: FileTypeDef = FileTypeDef {
	name: "scss",
	extensions: &["scss", "sass"],
	filenames: &[],
	first_line_patterns: &[],
	description: "SCSS/Sass file",
};

#[distributed_slice(FILE_TYPES)]
static FT_LESS: FileTypeDef = FileTypeDef {
	name: "less",
	extensions: &["less"],
	filenames: &[],
	first_line_patterns: &[],
	description: "Less file",
};
