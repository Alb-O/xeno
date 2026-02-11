use xeno_primitives::Color;

use super::*;

#[test]
fn test_highlight_styles() {
	let scopes = ["keyword", "string"];

	let styles = HighlightStyles::new(&scopes, |scope| match scope {
		"keyword" => Style::new().fg(Color::Red),
		"string" => Style::new().fg(Color::Green),
		_ => Style::new(),
	});

	assert_eq!(styles.len(), 2);
	assert_eq!(styles.style_for_highlight(Highlight::new(0)), Style::new().fg(Color::Red));
	assert_eq!(styles.style_for_highlight(Highlight::new(1)), Style::new().fg(Color::Green));
}

#[test]
fn test_highlight_span() {
	let span = HighlightSpan {
		start: 10,
		end: 20,
		highlight: Highlight::new(0),
	};

	assert_eq!(span.range(), 10..20);
	assert_eq!(span.len(), 10);
	assert!(!span.is_empty());
}
