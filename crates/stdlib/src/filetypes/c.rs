use tome_manifest::language;

language!(c, {
	extensions: &["c", "h"],
	description: "C source file",
});

language!(cpp, {
	extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx", "c++", "h++"],
	description: "C++ source file",
});
