use termina::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, Modifiers};
use xeno_primitives::Range;

use super::*;
use crate::impls::Editor;

#[test]
fn order_places_zero_last() {
	let mut tabstops = BTreeMap::new();
	#[allow(clippy::single_range_in_vec_init, reason = "these are single-element Vecs, not range expansions")]
	{
		tabstops.insert(0, vec![9..9]);
		tabstops.insert(3, vec![7..8]);
		tabstops.insert(1, vec![3..4]);
	}

	assert_eq!(tabstop_order(&tabstops), vec![1, 3, 0]);
}

#[test]
fn normalize_ranges_sorts_and_merges() {
	let ranges = vec![5..8, 1..3, 3..6, 10..11];
	assert_eq!(normalize_ranges(ranges), vec![1..3, 3..8, 10..11]);
}

#[test]
fn normalize_ranges_does_not_merge_adjacent() {
	let ranges = vec![0..1, 1..2];
	assert_eq!(normalize_ranges(ranges), vec![0..1, 1..2]);
}

#[test]
fn normalize_ranges_dedups_points() {
	let ranges = vec![5..5, 5..5];
	assert_eq!(normalize_ranges(ranges), vec![5..5]);
}

fn key_tab() -> KeyEvent {
	KeyEvent {
		code: KeyCode::Tab,
		modifiers: Modifiers::NONE,
		kind: KeyEventKind::Press,
		state: KeyEventState::NONE,
	}
}

fn key_char(c: char) -> KeyEvent {
	KeyEvent {
		code: KeyCode::Char(c),
		modifiers: Modifiers::NONE,
		kind: KeyEventKind::Press,
		state: KeyEventState::NONE,
	}
}

fn key_ctrl(c: char) -> KeyEvent {
	KeyEvent {
		code: KeyCode::Char(c),
		modifiers: Modifiers::CONTROL,
		kind: KeyEventKind::Press,
		state: KeyEventState::NONE,
	}
}

fn key_ctrl_space() -> KeyEvent {
	KeyEvent {
		code: KeyCode::Char(' '),
		modifiers: Modifiers::CONTROL,
		kind: KeyEventKind::Press,
		state: KeyEventState::NONE,
	}
}

fn key_enter() -> KeyEvent {
	KeyEvent {
		code: KeyCode::Enter,
		modifiers: Modifiers::NONE,
		kind: KeyEventKind::Press,
		state: KeyEventState::NONE,
	}
}

fn key_escape() -> KeyEvent {
	KeyEvent {
		code: KeyCode::Escape,
		modifiers: Modifiers::NONE,
		kind: KeyEventKind::Press,
		state: KeyEventState::NONE,
	}
}

fn buffer_text(editor: &Editor) -> String {
	editor.buffer().with_doc(|doc| doc.content().to_string())
}

fn primary_text(editor: &Editor) -> String {
	let range = editor.buffer().selection.primary();
	editor.buffer().with_doc(|doc| {
		let (from, to) = range.extent_clamped(doc.content().len_chars());
		doc.content().slice(from..to).to_string()
	})
}

fn set_multicursor_points(editor: &mut Editor, points: &[CharIdx]) {
	assert!(!points.is_empty(), "points must be non-empty");
	let primary = Range::point(points[0]);
	let others = points.iter().skip(1).copied().map(Range::point);
	let selection = Selection::new(primary, others);
	editor.buffer_mut().set_cursor_and_selection(points[0], selection);
}

fn set_multicursor_ranges(editor: &mut Editor, ranges: &[(CharIdx, CharIdx)]) {
	assert!(!ranges.is_empty(), "ranges must be non-empty");
	let primary = Range::from_exclusive(ranges[0].0, ranges[0].1);
	let others = ranges.iter().skip(1).map(|(start, end)| Range::from_exclusive(*start, *end));
	let selection = Selection::new(primary, others);
	editor.buffer_mut().set_cursor_and_selection(ranges[0].1, selection);
}

#[tokio::test]
async fn insert_snippet_body_starts_session_and_selects_first_placeholder() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("a ${1:x} b ${2:y} c $0"));
	assert_eq!(buffer_text(&editor), "a x b y c ");
	assert_eq!(primary_text(&editor), "x");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_some()
	);
}

#[tokio::test]
async fn insert_snippet_body_allows_multichar_and_tab_flow() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1:x} ${2:y} $0"));
	assert_eq!(buffer_text(&editor), "x y ");
	assert_eq!(primary_text(&editor), "x");

	let _ = editor.handle_key(key_char('Q')).await;
	assert_eq!(buffer_text(&editor), "Q y ");
	let _ = editor.handle_key(key_char('W')).await;
	assert_eq!(buffer_text(&editor), "QW y ");

	assert!(editor.handle_snippet_session_key(&key_tab()));
	assert_eq!(primary_text(&editor), "y");

	let _ = editor.handle_key(key_char('Z')).await;
	assert_eq!(buffer_text(&editor), "QW Z ");

	assert!(editor.handle_snippet_session_key(&key_tab()));
	assert_eq!(primary_text(&editor), "");

	assert!(editor.handle_snippet_session_key(&key_tab()));
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_none()
	);
}

#[tokio::test]
async fn insert_snippet_body_adjacent_mirrors_do_not_merge() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1:a}${1:b}"));
	assert_eq!(buffer_text(&editor), "ab");
	assert_eq!(editor.buffer().selection.len(), 2);
	assert_eq!(primary_text(&editor), "a");

	let _ = editor.handle_key(key_char('X')).await;
	assert_eq!(buffer_text(&editor), "XX");
}

#[tokio::test]
async fn snippet_insert_respects_moved_caret_in_active_tabstop() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1:abcd}$0"));
	let _ = editor.handle_key(key_char('Q')).await;
	let _ = editor.handle_key(key_char('W')).await;
	assert_eq!(buffer_text(&editor), "QW");

	editor.buffer_mut().set_cursor_and_selection(1, Selection::point(1));
	let _ = editor.handle_key(key_char('X')).await;
	assert_eq!(buffer_text(&editor), "QXW");
}

#[tokio::test]
async fn snippet_paste_replaces_placeholder_and_flips_to_insert_mode() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1:abcd}"));
	editor.handle_paste("XYZ".to_string());
	assert_eq!(buffer_text(&editor), "XYZ");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_some_and(|session| session.active_mode == ActiveMode::Insert)
	);

	let _ = editor.handle_key(key_char('Q')).await;
	assert_eq!(buffer_text(&editor), "XYZQ");
}

#[tokio::test]
async fn session_cancels_on_keyboard_cursor_move_outside_active_ranges() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("aa${1:abcd}bb$0"));
	editor.buffer_mut().set_cursor_and_selection(0, Selection::point(0));
	editor.snippet_session_on_cursor_moved(editor.focused_view());
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_none()
	);
}

#[tokio::test]
async fn session_survives_cursor_move_within_active_range() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("aa${1:abcd}bb$0"));
	editor.buffer_mut().set_cursor_and_selection(3, Selection::point(3));
	editor.snippet_session_on_cursor_moved(editor.focused_view());
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_some()
	);
}

#[tokio::test]
async fn insert_snippet_body_choice_cycles() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1|a,b,c|} $0"));
	assert_eq!(buffer_text(&editor), "a ");
	assert_eq!(primary_text(&editor), "a");

	assert!(editor.handle_snippet_session_key(&key_ctrl('n')));
	assert_eq!(buffer_text(&editor), "b ");
	assert_eq!(editor.buffer().selection.primary().head, 1);

	assert!(editor.handle_snippet_session_key(&key_ctrl('p')));
	assert_eq!(buffer_text(&editor), "a ");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_some()
	);
}

#[tokio::test]
async fn choice_overlay_open_and_commit_replaces_all_occurrences() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1|a,b,c|} ${1|a,b,c|} $0"));
	assert_eq!(buffer_text(&editor), "a a ");
	assert!(editor.handle_snippet_session_key(&key_ctrl_space()));
	{
		let overlay = editor.overlays_mut().get_or_default::<SnippetChoiceOverlay>();
		assert!(overlay.active);
		overlay.selected = 2;
	}

	assert!(editor.handle_snippet_session_key(&key_enter()));
	assert_eq!(buffer_text(&editor), "c c ");
	assert!(
		editor
			.overlays()
			.get::<SnippetChoiceOverlay>()
			.is_some_and(|overlay| !overlay.active)
	);
}

#[tokio::test]
async fn choice_overlay_escape_noops() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1|a,b|} $0"));
	assert_eq!(buffer_text(&editor), "a ");
	assert!(editor.handle_snippet_session_key(&key_ctrl_space()));
	assert!(editor.handle_snippet_session_key(&key_escape()));
	assert_eq!(buffer_text(&editor), "a ");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_some()
	);
}

#[tokio::test]
async fn choice_cycle_updates_transform_without_tab() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1|a,b|} ${1/(.*)/$1_bar/} $0"));
	assert_eq!(buffer_text(&editor), "a a_bar ");

	assert!(editor.handle_snippet_session_key(&key_ctrl('n')));
	assert_eq!(buffer_text(&editor), "b b_bar ");
}

#[tokio::test]
async fn choice_overlay_commit_updates_transform_without_tab() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1|a,b|} ${1/(.*)/$1_bar/} $0"));
	assert_eq!(buffer_text(&editor), "a a_bar ");

	assert!(editor.handle_snippet_session_key(&key_ctrl_space()));
	{
		let overlay = editor.overlays_mut().get_or_default::<SnippetChoiceOverlay>();
		overlay.selected = 1;
	}
	assert!(editor.handle_snippet_session_key(&key_enter()));
	assert_eq!(buffer_text(&editor), "b b_bar ");
}

#[tokio::test]
async fn insert_snippet_body_choice_cycles_mirrors() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1|a,b|} ${1|a,b|} $0"));
	assert_eq!(buffer_text(&editor), "a a ");
	assert_eq!(editor.buffer().selection.len(), 2);

	assert!(editor.handle_snippet_session_key(&key_ctrl('n')));
	assert_eq!(buffer_text(&editor), "b b ");
}

#[tokio::test]
async fn insert_snippet_body_choice_cycles_with_multicursor() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);
	editor.buffer_mut().reset_content("1\n2\n");
	set_multicursor_points(&mut editor, &[0, 2]);

	assert!(editor.insert_snippet_body("${1|x,y|} $0"));
	assert_eq!(buffer_text(&editor), "x 1\nx 2\n");
	assert_eq!(editor.buffer().selection.len(), 2);

	assert!(editor.handle_snippet_session_key(&key_ctrl('n')));
	assert_eq!(buffer_text(&editor), "y 1\ny 2\n");
}

#[tokio::test]
async fn snippet_command_named_lookup_inserts_and_starts_session() {
	use crate::types::InvocationResult;

	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	let result = editor.invoke_command("snippet", vec!["@fori".to_string()]).await;
	assert!(matches!(result, InvocationResult::Ok));
	assert_eq!(buffer_text(&editor), "for i in 0..n {\n\t\n}");
	assert_eq!(primary_text(&editor), "i");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_some()
	);
}

#[tokio::test]
async fn insert_snippet_body_multicursor_points_starts_one_session() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);
	editor.buffer_mut().reset_content("a\nb\n");
	set_multicursor_points(&mut editor, &[0, 2]);

	assert!(editor.insert_snippet_body("${1:x} $0"));
	assert_eq!(buffer_text(&editor), "x a\nx b\n");
	assert_eq!(primary_text(&editor), "x");
	assert_eq!(editor.buffer().selection.len(), 2);
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_some()
	);

	let _ = editor.handle_key(key_char('Q')).await;
	assert_eq!(buffer_text(&editor), "Q a\nQ b\n");
	let _ = editor.handle_key(key_char('W')).await;
	assert_eq!(buffer_text(&editor), "QW a\nQW b\n");

	assert!(editor.handle_snippet_session_key(&key_tab()));
	assert_eq!(editor.buffer().selection.len(), 2);
	assert!(editor.handle_snippet_session_key(&key_tab()));
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_none()
	);
}

#[tokio::test]
async fn insert_snippet_body_multicursor_no_tabstops_sets_points_and_no_session() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);
	editor.buffer_mut().reset_content("a\nb\n");
	set_multicursor_points(&mut editor, &[0, 2]);

	assert!(editor.insert_snippet_body("hello"));
	assert_eq!(buffer_text(&editor), "helloa\nhellob\n");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_none()
	);
	assert_eq!(editor.buffer().selection.len(), 2);
	let points: Vec<CharIdx> = editor.buffer().selection.iter().map(|range| range.head).collect();
	assert_eq!(points, vec![5, 12]);
}

#[tokio::test]
async fn insert_snippet_body_selection_variable_uses_primary_selection() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);
	editor.buffer_mut().reset_content("hello world");
	editor
		.buffer_mut()
		.set_cursor_and_selection(5, Selection::new(Range::from_exclusive(0, 5), std::iter::empty()));

	assert!(editor.insert_snippet_body("$SELECTION"));
	assert_eq!(buffer_text(&editor), "hello world");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_none()
	);
}

#[tokio::test]
async fn insert_snippet_body_tm_selected_text_alias_uses_primary_selection() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);
	editor.buffer_mut().reset_content("hello world");
	editor
		.buffer_mut()
		.set_cursor_and_selection(5, Selection::new(Range::from_exclusive(0, 5), std::iter::empty()));

	assert!(editor.insert_snippet_body("$TM_SELECTED_TEXT"));
	assert_eq!(buffer_text(&editor), "hello world");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_none()
	);
}

#[tokio::test]
async fn insert_snippet_body_malformed_transform_remains_literal_and_keeps_session() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("a ${1:x} ${1/(.*)/$1"));
	assert_eq!(buffer_text(&editor), "a x ${1/(.*)/$1");
	assert_eq!(primary_text(&editor), "x");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_some()
	);

	assert!(editor.handle_snippet_session_key(&key_tab()));
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_none()
	);
}

#[tokio::test]
async fn insert_snippet_body_selection_variable_expands_per_selection() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);
	editor.buffer_mut().reset_content("hello world");
	set_multicursor_ranges(&mut editor, &[(0, 5), (6, 11)]);

	assert!(editor.insert_snippet_body("$SELECTION"));
	assert_eq!(buffer_text(&editor), "hello world");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_none()
	);
	let points: Vec<CharIdx> = editor.buffer().selection.iter().map(|range| range.head).collect();
	assert_eq!(points, vec![5, 11]);
}

#[tokio::test]
async fn insert_snippet_body_selection_variable_expands_per_selection_with_tabstop() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);
	editor.buffer_mut().reset_content("hello world");
	set_multicursor_ranges(&mut editor, &[(0, 5), (6, 11)]);

	assert!(editor.insert_snippet_body("(${SELECTION})$0"));
	assert_eq!(buffer_text(&editor), "(hello) (world)");
	assert!(
		editor
			.overlays()
			.get::<SnippetSessionState>()
			.and_then(|state| state.session.as_ref())
			.is_some()
	);
	let points: Vec<CharIdx> = editor.buffer().selection.iter().map(|range| range.head).collect();
	assert_eq!(points, vec![7, 15]);
}

#[tokio::test]
async fn insert_snippet_body_current_second_uses_single_timestamp_across_cursors() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);
	editor.buffer_mut().reset_content("\n\n");
	set_multicursor_points(&mut editor, &[0, 1]);

	assert!(editor.insert_snippet_body("${CURRENT_SECOND}|${CURRENT_SECOND}"));
	let text = buffer_text(&editor);
	let lines: Vec<&str> = text.lines().collect();
	assert_eq!(lines.len(), 2);
	for line in &lines {
		let parts: Vec<&str> = line.split('|').collect();
		assert_eq!(parts.len(), 2);
		assert_eq!(parts[0], parts[1]);
		assert_eq!(parts[0].len(), 2);
		assert!(parts[0].chars().all(|ch| ch.is_ascii_digit()));
	}
	assert_eq!(lines[0], lines[1]);
}

#[tokio::test]
async fn tabstop_transform_updates_on_tab() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1:foo} ${1/(.*)/$1_bar/} $0"));
	assert_eq!(buffer_text(&editor), "foo foo_bar ");
	assert_eq!(primary_text(&editor), "foo");

	let _ = editor.handle_key(key_char('x')).await;
	assert_eq!(buffer_text(&editor), "x x_bar ");

	assert!(editor.handle_snippet_session_key(&key_tab()));
	assert_eq!(buffer_text(&editor), "x x_bar ");
	assert_eq!(primary_text(&editor), "");
}

#[tokio::test]
async fn tabstop_transform_updates_while_typing_without_tab() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1:foo} ${1/(.*)/$1_bar/} $0"));
	assert_eq!(buffer_text(&editor), "foo foo_bar ");

	let _ = editor.handle_key(key_char('x')).await;
	assert_eq!(buffer_text(&editor), "x x_bar ");

	let _ = editor.handle_key(key_char('y')).await;
	assert_eq!(buffer_text(&editor), "xy xy_bar ");
}

#[tokio::test]
async fn transform_apply_does_not_recurse_or_break_session() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);

	assert!(editor.insert_snippet_body("${1:foo} ${1/(.*)/$1_bar/} $0"));
	let _ = editor.handle_key(key_char('x')).await;
	let _ = editor.handle_key(key_char('y')).await;
	assert_eq!(buffer_text(&editor), "xy xy_bar ");

	let session = editor
		.overlays()
		.get::<SnippetSessionState>()
		.and_then(|state| state.session.as_ref())
		.expect("snippet session should stay active");
	assert_eq!(session.active_tabstop(), Some(1));
}

#[tokio::test]
async fn tabstop_transform_updates_per_selection_instance_on_tab() {
	let mut editor = Editor::new_scratch();
	editor.set_mode(Mode::Insert);
	editor.buffer_mut().reset_content("foo\nbar\n");
	set_multicursor_ranges(&mut editor, &[(0, 3), (4, 7)]);

	assert!(editor.insert_snippet_body("${1:${SELECTION}} ${1/(.*)/$1_bar/} $0"));
	assert_eq!(buffer_text(&editor), "foo foo_bar \nbar bar_bar \n");

	assert!(editor.handle_snippet_session_key(&key_tab()));
	assert_eq!(buffer_text(&editor), "foo foo_bar \nbar bar_bar \n");
	assert_eq!(editor.buffer().selection.len(), 2);
}

#[cfg(feature = "lsp")]
mod lsp_tests {
	use xeno_lsp::lsp_types::{CompletionItem, InsertTextFormat};

	use super::*;

	#[tokio::test]
	async fn lsp_snippet_session_tab_flow() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		let buffer_id = editor.focused_view();

		let item = CompletionItem {
			label: "snippet".to_string(),
			insert_text: Some("a ${1:x} b ${2:y} c $0".to_string()),
			insert_text_format: Some(InsertTextFormat::SNIPPET),
			..Default::default()
		};

		editor.apply_completion_item(buffer_id, item).await;
		assert_eq!(buffer_text(&editor), "a x b y c ");
		assert_eq!(primary_text(&editor), "x");

		let _ = editor.handle_key(key_char('Q')).await;
		assert_eq!(buffer_text(&editor), "a Q b y c ");
		let _ = editor.handle_key(key_char('W')).await;
		assert_eq!(buffer_text(&editor), "a QW b y c ");

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert_eq!(primary_text(&editor), "y");

		let _ = editor.handle_key(key_char('Z')).await;
		assert_eq!(buffer_text(&editor), "a QW b Z c ");

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert_eq!(primary_text(&editor), "");

		assert!(editor.handle_snippet_session_key(&key_tab()));
		assert!(
			editor
				.overlays()
				.get::<SnippetSessionState>()
				.and_then(|state| state.session.as_ref())
				.is_none()
		);
		assert!(!editor.handle_snippet_session_key(&key_tab()));
	}

	#[tokio::test]
	async fn lsp_snippet_mirror_uses_multiselection_edit() {
		let mut editor = Editor::new_scratch();
		editor.set_mode(Mode::Insert);
		let buffer_id = editor.focused_view();

		let item = CompletionItem {
			label: "mirror".to_string(),
			insert_text: Some("${1:x}-$1".to_string()),
			insert_text_format: Some(InsertTextFormat::SNIPPET),
			..Default::default()
		};

		editor.apply_completion_item(buffer_id, item).await;
		assert_eq!(buffer_text(&editor), "x-");
		assert_eq!(editor.buffer().selection.len(), 2);

		let _ = editor.handle_key(key_char('Q')).await;
		assert_eq!(buffer_text(&editor), "Q-Q");
		let _ = editor.handle_key(key_char('W')).await;
		assert_eq!(buffer_text(&editor), "QW-QW");
	}
}
