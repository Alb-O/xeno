use crate::filetype;

filetype!(c, {
	extensions: &["c", "h"],
	description: "C source file",
});

filetype!(cpp, {
	extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx", "c++", "h++"],
	description: "C++ source file",
});
