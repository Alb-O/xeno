use tome_manifest::language;

language!(rust, {
	grammar: "rust",
	extensions: &["rs"],
	comment_tokens: &["//"],
	block_comment: ("/*", "*/"),
	description: "Rust source file",
});
