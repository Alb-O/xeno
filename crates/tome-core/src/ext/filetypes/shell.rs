use linkme::distributed_slice;

use crate::ext::{FILE_TYPES, FileTypeDef};

#[distributed_slice(FILE_TYPES)]
static FT_SH: FileTypeDef = FileTypeDef {
	name: "sh",
	extensions: &["sh"],
	filenames: &[".bashrc", ".profile", ".bash_profile", ".bash_logout"],
	first_line_patterns: &["#!/bin/sh"],
	description: "POSIX shell script",
};

#[distributed_slice(FILE_TYPES)]
static FT_BASH: FileTypeDef = FileTypeDef {
	name: "bash",
	extensions: &["bash"],
	filenames: &[".bashrc", ".bash_profile", ".bash_logout"],
	first_line_patterns: &["#!/bin/bash", "#!/usr/bin/env bash"],
	description: "Bash script",
};

#[distributed_slice(FILE_TYPES)]
static FT_ZSH: FileTypeDef = FileTypeDef {
	name: "zsh",
	extensions: &["zsh"],
	filenames: &[".zshrc", ".zprofile", ".zshenv", ".zlogout"],
	first_line_patterns: &["#!/bin/zsh", "#!/usr/bin/env zsh"],
	description: "Zsh script",
};

#[distributed_slice(FILE_TYPES)]
static FT_FISH: FileTypeDef = FileTypeDef {
	name: "fish",
	extensions: &["fish"],
	filenames: &[],
	first_line_patterns: &["#!/usr/bin/env fish"],
	description: "Fish script",
};

#[distributed_slice(FILE_TYPES)]
static FT_NU: FileTypeDef = FileTypeDef {
	name: "nu",
	extensions: &["nu"],
	filenames: &[],
	first_line_patterns: &["#!/usr/bin/env nu"],
	description: "Nushell script",
};
