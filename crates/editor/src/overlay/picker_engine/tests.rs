use crate::completion::{CompletionItem, CompletionKind, CompletionState, SelectionIntent};

#[test]
fn parser_tokenize_preserves_quoted_bounds() {
	let chars: Vec<char> = "open \"My File\"".chars().collect();
	let tokens = super::parser::tokenize(&chars);
	assert_eq!(tokens.len(), 2);
	assert_eq!(tokens[1].quoted, Some('"'));
	assert_eq!(tokens[1].close_quote_idx, Some(13));
}

#[test]
fn decision_selected_item_requires_active_state() {
	let mut state = CompletionState::default();
	state.items = vec![CompletionItem {
		label: "write".to_string(),
		insert_text: "write".to_string(),
		detail: None,
		filter_text: None,
		kind: CompletionKind::Command,
		match_indices: None,
		right: None,
	}];
	state.selected_idx = Some(0);
	state.selection_intent = SelectionIntent::Manual;

	assert!(super::decision::selected_completion_item(Some(&state)).is_none());
	state.active = true;
	assert_eq!(
		super::decision::selected_completion_item(Some(&state))
			.as_ref()
			.map(|item| item.insert_text.as_str()),
		Some("write")
	);
}

#[test]
fn apply_replace_char_range_is_unicode_safe() {
	let (out, cursor) = super::apply::replace_char_range("ab界d", 1, 3, "ZZ");
	assert_eq!(out, "aZZd");
	assert_eq!(cursor, 3);
}
