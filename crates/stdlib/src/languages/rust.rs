use crate::language;

language!(rust, {
	extensions: &["rs"],
	comment_tokens: &["//"],
	block_comment: ("/*", "*/"),
	description: "Rust programming language",
});
