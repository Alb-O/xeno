use crate::filetype;

filetype!(nix, {
	extensions: &["nix"],
	description: "Nix expression",
});

filetype!(makefile, {
	extensions: &["mk"],
	filenames: &["Makefile", "makefile", "GNUmakefile"],
	description: "Makefile",
});

filetype!(dockerfile, {
	extensions: &[],
	filenames: &["Dockerfile", "Containerfile"],
	description: "Dockerfile",
});

filetype!(gitignore, {
	extensions: &["gitignore"],
	filenames: &[".gitignore", ".gitattributes", ".gitmodules"],
	description: "Git config file",
});

filetype!(editorconfig, {
	extensions: &[],
	filenames: &[".editorconfig"],
	description: "EditorConfig file",
});

filetype!(ini, {
	extensions: &["ini", "cfg", "conf"],
	description: "INI config file",
});

filetype!(env, {
	extensions: &["env"],
	filenames: &[
		".env",
		".envrc",
		".env.local",
		".env.development",
		".env.production",
	],
	description: "Environment file",
});
