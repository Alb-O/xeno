use xeno_primitives::{Key, KeyCode};

use super::{LspMenuKind, LspMenuState, lsp_completion_raw_index};
use crate::Editor;
use crate::completion::{CompletionItem, CompletionState};
use crate::render_api::CompletionKind;

fn key_tab() -> Key {
	Key::new(KeyCode::Tab)
}

#[test]
fn uses_display_to_raw_mapping_when_present() {
	let state = CompletionState {
		lsp_display_to_raw: vec![2, 0, 1],
		..Default::default()
	};

	assert_eq!(lsp_completion_raw_index(Some(&state), 0), 2);
	assert_eq!(lsp_completion_raw_index(Some(&state), 1), 0);
	assert_eq!(lsp_completion_raw_index(Some(&state), 2), 1);
}

#[test]
fn falls_back_to_display_index_when_mapping_missing() {
	let state = CompletionState::default();

	assert_eq!(lsp_completion_raw_index(Some(&state), 3), 3);
	assert_eq!(lsp_completion_raw_index(None, 1), 1);
}

#[tokio::test]
async fn tab_accept_uses_display_to_raw_mapping_for_lsp_completions() {
	let mut editor = Editor::new_scratch();
	let buffer_id = editor.focused_view();

	let raw_items = vec![
		xeno_lsp::lsp_types::CompletionItem {
			label: "alpha".to_string(),
			insert_text: Some("alpha".to_string()),
			..Default::default()
		},
		xeno_lsp::lsp_types::CompletionItem {
			label: "beta".to_string(),
			insert_text: Some("beta".to_string()),
			..Default::default()
		},
	];

	let completion_state = editor.overlays_mut().get_or_default::<CompletionState>();
	completion_state.active = true;
	completion_state.items = vec![CompletionItem {
		label: "beta".to_string(),
		insert_text: "beta".to_string(),
		detail: None,
		filter_text: None,
		kind: CompletionKind::Command,
		match_indices: None,
		right: None,
	}];
	completion_state.lsp_display_to_raw = vec![1];
	completion_state.selected_idx = Some(0);
	completion_state.replace_start = 0;

	let menu_state = editor.overlays_mut().get_or_default::<LspMenuState>();
	menu_state.set(LspMenuKind::Completion { buffer_id, items: raw_items });

	let consumed = editor.handle_lsp_menu_key(&key_tab()).await;
	assert!(consumed);

	let content = editor.buffer().with_doc(|doc| doc.content().to_string());
	assert_eq!(content, "beta");
}
