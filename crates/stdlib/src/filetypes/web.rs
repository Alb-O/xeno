use crate::filetype;

filetype!(html, {
	extensions: &["html", "htm", "xhtml"],
	first_line_patterns: &["<!DOCTYPE html", "<!doctype html"],
	description: "HTML file",
});

filetype!(css, {
	extensions: &["css"],
	description: "CSS file",
});

filetype!(scss, {
	extensions: &["scss", "sass"],
	description: "SCSS/Sass file",
});

filetype!(less, {
	extensions: &["less"],
	description: "Less file",
});
