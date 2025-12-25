use tome_manifest::language;

language!(javascript, {
	extensions: &["js", "mjs", "cjs"],
	first_line_patterns: &["node"],
	description: "JavaScript source file",
});

language!(jsx, {
	extensions: &["jsx"],
	description: "JavaScript JSX file",
});
