use crate::filetype;

filetype!(markdown, {
	extensions: &["md", "markdown", "mkd", "mkdn"],
	filenames: &["README", "CHANGELOG", "CONTRIBUTING", "LICENSE"],
	description: "Markdown file",
});

filetype!(rst, {
	extensions: &["rst", "rest"],
	description: "reStructuredText file",
});

filetype!(asciidoc, {
	extensions: &["adoc", "asciidoc", "asc"],
	description: "AsciiDoc file",
});

filetype!(tex, {
	extensions: &["tex", "sty", "cls"],
	description: "LaTeX file",
});
