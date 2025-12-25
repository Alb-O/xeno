use tome_manifest::language;

language!(markdown, {
	extensions: &["md", "markdown", "mkd", "mkdn"],
	filenames: &["README", "CHANGELOG", "CONTRIBUTING", "LICENSE"],
	description: "Markdown file",
});

language!(rst, {
	extensions: &["rst", "rest"],
	description: "reStructuredText file",
});

language!(asciidoc, {
	extensions: &["adoc", "asciidoc", "asc"],
	description: "AsciiDoc file",
});

language!(tex, {
	extensions: &["tex", "sty", "cls"],
	description: "LaTeX file",
});
