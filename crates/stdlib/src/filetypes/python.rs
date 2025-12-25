use crate::filetype;

filetype!(python, {
	extensions: &["py", "pyw", "pyi"],
	first_line_patterns: &["python", "python3"],
	description: "Python source file",
});
