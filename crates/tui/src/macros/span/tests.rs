use crate::style::{Color, Modifier, Style};
use crate::text::Span;

#[test]
fn raw() {
	let test = "test";
	let content = "content";
	let number = 123;

	let span = span!("test content");
	assert_eq!(span, Span::raw("test content"));

	let span = span!("test {}", "content");
	assert_eq!(span, Span::raw("test content"));

	let span = span!("test {}", content);
	assert_eq!(span, Span::raw("test content"));

	let span = span!("test {content}");
	assert_eq!(span, Span::raw("test content"));

	let span = span!("test {content}", content = "content");
	assert_eq!(span, Span::raw("test content"));

	let span = span!("test {content}", content = content);
	assert_eq!(span, Span::raw("test content"));

	let span = span!("{} {}", "test", "content");
	assert_eq!(span, Span::raw("test content"));

	let span = span!("{test} {content}");
	assert_eq!(span, Span::raw("test content"));

	let span = span!("test {number}");
	assert_eq!(span, Span::raw("test 123"));

	// a number with a format specifier
	let span = span!("test {number:04}");
	assert_eq!(span, Span::raw("test 0123"));

	// directly pass a number expression
	let span = span!(number);
	assert_eq!(span, Span::raw("123"));

	// directly pass a string expression
	let span = span!(test);
	assert_eq!(span, Span::raw("test"));
}

#[test]
fn styled() {
	const STYLE: Style = Style::new().fg(Color::Green);

	let test = "test";
	let content = "content";
	let number = 123;

	let span = span!(STYLE; "test content");
	assert_eq!(span, Span::styled("test content", STYLE));

	let span = span!(STYLE; "test {}", "content");
	assert_eq!(span, Span::styled("test content", STYLE));

	let span = span!(STYLE; "test {}", content);
	assert_eq!(span, Span::styled("test content", STYLE));

	let span = span!(STYLE; "test {content}");
	assert_eq!(span, Span::styled("test content", STYLE));

	let span = span!(STYLE; "test {content}", content = "content");
	assert_eq!(span, Span::styled("test content", STYLE));

	let span = span!(STYLE; "test {content}", content = content);
	assert_eq!(span, Span::styled("test content", STYLE));

	let span = span!(STYLE; "{} {}", "test", "content");
	assert_eq!(span, Span::styled("test content", STYLE));

	let span = span!(STYLE; "{test} {content}");
	assert_eq!(span, Span::styled("test content", STYLE));

	let span = span!(STYLE; "test {number}");
	assert_eq!(span, Span::styled("test 123", STYLE));

	// a number with a format specifier
	let span = span!(STYLE; "test {number:04}");
	assert_eq!(span, Span::styled("test 0123", STYLE));

	// accepts any type that is convertible to Style
	let span = span!(Color::Green; "test {content}");
	assert_eq!(span, Span::styled("test content", STYLE));

	let span = span!(Modifier::BOLD; "test {content}");
	assert_eq!(span, Span::styled("test content", Style::new().bold()));

	// directly pass a number expression
	let span = span!(STYLE; number);
	assert_eq!(span, Span::styled("123", STYLE));

	// directly pass a string expression
	let span = span!(STYLE; test);
	assert_eq!(span, Span::styled("test", STYLE));
}
