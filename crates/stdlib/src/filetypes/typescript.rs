use evildoer_manifest::language;

language!(typescript, {
	extensions: &["ts", "mts", "cts"],
	description: "TypeScript source file",
});

language!(tsx, {
	extensions: &["tsx"],
	description: "TypeScript JSX file",
});
