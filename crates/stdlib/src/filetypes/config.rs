use tome_manifest::language;

language!(nix, {
	extensions: &["nix"],
	description: "Nix expression",
});

language!(makefile, {
	extensions: &["mk"],
	filenames: &["Makefile", "makefile", "GNUmakefile"],
	description: "Makefile",
});

language!(dockerfile, {
	extensions: &[],
	filenames: &["Dockerfile", "Containerfile"],
	description: "Dockerfile",
});

language!(gitignore, {
	extensions: &["gitignore"],
	filenames: &[".gitignore", ".gitattributes", ".gitmodules"],
	description: "Git config file",
});

language!(editorconfig, {
	extensions: &[],
	filenames: &[".editorconfig"],
	description: "EditorConfig file",
});

language!(ini, {
	extensions: &["ini", "cfg", "conf"],
	description: "INI config file",
});

language!(env, {
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
