//! Span operator and debug tests

use rstest::rstest;

use super::*;

#[test]
fn add() {
	assert_eq!(Span::default() + Span::default(), Line::from(vec![Span::default(), Span::default()]));

	assert_eq!(Span::default() + Span::raw("test"), Line::from(vec![Span::default(), Span::raw("test")]));

	assert_eq!(Span::raw("test") + Span::default(), Line::from(vec![Span::raw("test"), Span::default()]));

	assert_eq!(
		Span::raw("test") + Span::raw("content"),
		Line::from(vec![Span::raw("test"), Span::raw("content")])
	);
}

#[rstest]
#[case::default(Span::default(), "Span::default()")]
#[case::raw(Span::raw("test"), r#"Span::from("test")"#)]
#[case::styled(Span::styled("test", Style::new().green()), r#"Span::from("test").green()"#)]
#[case::styled_italic(
    Span::styled("test", Style::new().green().italic()),
    r#"Span::from("test").green().italic()"#
)]
fn debug(#[case] span: Span, #[case] expected: &str) {
	assert_eq!(format!("{span:?}"), expected);
}
