use tome_manifest::language;

language!(html, {
	extensions: &["html", "htm", "xhtml"],
	first_line_patterns: &["<!DOCTYPE html", "<!doctype html"],
	description: "HTML file",
});

language!(css, {
	extensions: &["css"],
	description: "CSS file",
});

language!(scss, {
	extensions: &["scss", "sass"],
	description: "SCSS/Sass file",
});

language!(less, {
	extensions: &["less"],
	description: "Less file",
});
