use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_NIX: FileTypeDef = FileTypeDef {
	name: "nix",
	extensions: &["nix"],
	filenames: &[],
	first_line_patterns: &[],
	description: "Nix expression",
};

#[distributed_slice(FILE_TYPES)]
static FT_MAKEFILE: FileTypeDef = FileTypeDef {
	name: "makefile",
	extensions: &["mk"],
	filenames: &["Makefile", "makefile", "GNUmakefile"],
	first_line_patterns: &[],
	description: "Makefile",
};

#[distributed_slice(FILE_TYPES)]
static FT_DOCKERFILE: FileTypeDef = FileTypeDef {
	name: "dockerfile",
	extensions: &[],
	filenames: &["Dockerfile", "Containerfile"],
	first_line_patterns: &[],
	description: "Dockerfile",
};

#[distributed_slice(FILE_TYPES)]
static FT_GITIGNORE: FileTypeDef = FileTypeDef {
	name: "gitignore",
	extensions: &["gitignore"],
	filenames: &[".gitignore", ".gitattributes", ".gitmodules"],
	first_line_patterns: &[],
	description: "Git config file",
};

#[distributed_slice(FILE_TYPES)]
static FT_EDITORCONFIG: FileTypeDef = FileTypeDef {
	name: "editorconfig",
	extensions: &[],
	filenames: &[".editorconfig"],
	first_line_patterns: &[],
	description: "EditorConfig file",
};

#[distributed_slice(FILE_TYPES)]
static FT_INI: FileTypeDef = FileTypeDef {
	name: "ini",
	extensions: &["ini", "cfg", "conf"],
	filenames: &[],
	first_line_patterns: &[],
	description: "INI config file",
};

#[distributed_slice(FILE_TYPES)]
static FT_ENV: FileTypeDef = FileTypeDef {
	name: "env",
	extensions: &["env"],
	filenames: &[
		".env",
		".envrc",
		".env.local",
		".env.development",
		".env.production",
	],
	first_line_patterns: &[],
	description: "Environment file",
};
