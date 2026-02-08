use super::*;

#[test]
fn test_resolve_exact_match() {
	let mut styles = SyntaxStyles::minimal();
	styles.keyword = SyntaxStyle::fg(Color::Red);

	let resolved = styles.resolve("keyword");
	assert_eq!(resolved.fg, Some(Color::Red));
}

#[test]
fn test_resolve_hierarchical_fallback() {
	let mut styles = SyntaxStyles::minimal();
	styles.keyword = SyntaxStyle::fg(Color::Red);

	let resolved = styles.resolve("keyword.control.import");
	assert_eq!(resolved.fg, Some(Color::Red));
}

#[test]
fn test_resolve_partial_hierarchy() {
	let mut styles = SyntaxStyles::minimal();
	styles.keyword = SyntaxStyle::fg(Color::Red);
	styles.keyword_control = SyntaxStyle::fg(Color::Blue);

	let resolved = styles.resolve("keyword.control.import");
	assert_eq!(resolved.fg, Some(Color::Blue));
}

#[test]
fn test_scope_names_complete() {
	let names = SyntaxStyles::scope_names();
	assert!(names.contains(&"keyword"));
	assert!(names.contains(&"function.macro"));
	assert!(names.contains(&"variable.other.member"));
}
