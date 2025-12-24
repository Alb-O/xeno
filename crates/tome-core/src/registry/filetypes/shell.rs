use crate::filetype;

filetype!(sh, {
	extensions: &["sh"],
	filenames: &[".profile"],
	first_line_patterns: &["#!/bin/sh"],
	description: "POSIX shell script",
});

filetype!(bash, {
	extensions: &["bash"],
	filenames: &[".bashrc", ".bash_profile", ".bash_logout"],
	first_line_patterns: &["#!/bin/bash", "#!/usr/bin/env bash"],
	description: "Bash script",
	priority: 10,
});

filetype!(zsh, {
	extensions: &["zsh"],
	filenames: &[".zshrc", ".zprofile", ".zshenv", ".zlogout"],
	first_line_patterns: &["#!/bin/zsh", "#!/usr/bin/env zsh"],
	description: "Zsh script",
	priority: 10,
});

filetype!(fish, {
	extensions: &["fish"],
	first_line_patterns: &["#!/usr/bin/env fish"],
	description: "Fish script",
	priority: 10,
});

filetype!(nu, {
	extensions: &["nu"],
	first_line_patterns: &["#!/usr/bin/env nu"],
	description: "Nushell script",
	priority: 10,
});
