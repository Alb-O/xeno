use crate::filetype;

filetype!(typescript, {
	extensions: &["ts", "mts", "cts"],
	description: "TypeScript source file",
});

filetype!(tsx, {
	extensions: &["tsx"],
	description: "TypeScript JSX file",
});
