use super::*;

#[test]
fn test_language_data_creation() {
	let data = LanguageData::new(
		"rust".to_string(),
		None,
		vec!["rs".to_string()],
		vec!["Cargo.toml".to_string()],
		vec![],
		vec![],
		vec!["//".to_string()],
		Some(("/*".to_string(), "*/".to_string())),
		Some(r"^rust$"),
		vec!["rust-analyzer".to_string()],
		vec!["Cargo.toml".to_string()],
	);

	assert_eq!(data.name, "rust");
	assert_eq!(data.grammar_name, "rust");
	assert!(data.injection_regex.is_some());
	assert_eq!(data.lsp_servers, vec!["rust-analyzer"]);
	assert_eq!(data.roots, vec!["Cargo.toml"]);
}

#[test]
fn test_grammar_name_override() {
	let data = LanguageData::new(
		"typescript".to_string(),
		Some("tsx".to_string()),
		vec!["ts".to_string()],
		vec![],
		vec![],
		vec![],
		vec!["//".to_string()],
		None,
		None,
		vec![],
		vec![],
	);

	assert_eq!(data.name, "typescript");
	assert_eq!(data.grammar_name, "tsx");
}

/// Helper to test scope prefix matching (same algorithm as configure callback).
fn find_best_scope_match<'a>(capture_name: &str, scope_names: &[&'a str]) -> Option<&'a str> {
	let capture_parts: Vec<_> = capture_name.split('.').collect();

	let mut best_index = None;
	let mut best_match_len = 0;

	for (i, recognized_name) in scope_names.iter().enumerate() {
		let mut len = 0;
		let mut matches = true;

		for (j, part) in recognized_name.split('.').enumerate() {
			match capture_parts.get(j) {
				Some(&capture_part) if capture_part == part => len += 1,
				_ => {
					matches = false;
					break;
				}
			}
		}

		if matches && len > best_match_len {
			best_index = Some(i);
			best_match_len = len;
		}
	}

	best_index.map(|i| scope_names[i])
}

#[test]
fn test_scope_prefix_matching() {
	let scopes = &[
		"keyword",
		"keyword.control",
		"keyword.control.import",
		"markup.heading",
		"markup.heading.1",
		"string",
		"string.special",
	];

	// Exact match
	assert_eq!(find_best_scope_match("keyword.control", scopes), Some("keyword.control"));

	// Prefix match - more specific capture falls back to less specific scope
	assert_eq!(find_best_scope_match("keyword.control.flow", scopes), Some("keyword.control"));

	// Longer prefix wins
	assert_eq!(find_best_scope_match("keyword.control.import.default", scopes), Some("keyword.control.import"));

	// Falls back to base scope
	assert_eq!(find_best_scope_match("keyword.operator", scopes), Some("keyword"));

	// No match at all
	assert_eq!(find_best_scope_match("comment", scopes), None);

	// Markup test
	assert_eq!(find_best_scope_match("markup.heading.marker", scopes), Some("markup.heading"));

	// String special paths
	assert_eq!(find_best_scope_match("string.special.path", scopes), Some("string.special"));
}
