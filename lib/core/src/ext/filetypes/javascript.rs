use crate::filetype;

filetype!(javascript, {
	extensions: &["js", "mjs", "cjs"],
	first_line_patterns: &["node"],
	description: "JavaScript source file",
});

filetype!(jsx, {
	extensions: &["jsx"],
	description: "JavaScript JSX file",
});
