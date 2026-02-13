use std::time::Duration;

use xeno_primitives::{Key, KeyCode, Mode};

use super::*;

async fn run_script(editor: &mut Editor, events: impl IntoIterator<Item = RuntimeEvent>) {
	for event in events {
		let _ = editor.on_event(event).await;
	}
}

#[tokio::test]
async fn test_on_event_implies_pump() {
	let mut editor = Editor::new_scratch();

	// Initial pump to clear startup state
	let _ = editor.pump().await;

	// Any event should trigger maintenance
	let ev = RuntimeEvent::Key(Key::char('i'));

	let dir = editor.on_event(ev).await;

	// Insert mode should set fast timeout
	assert_eq!(dir.poll_timeout, Some(Duration::from_millis(16)));
	assert_eq!(editor.mode(), Mode::Insert);
}

#[tokio::test]
async fn test_runtime_event_scripts_converge_for_inserted_text() {
	let esc = Key::new(KeyCode::Esc);

	let script_with_paste = vec![
		RuntimeEvent::WindowResized { cols: 80, rows: 24 },
		RuntimeEvent::Key(Key::char('i')),
		RuntimeEvent::Paste(String::from("abc")),
		RuntimeEvent::Key(esc),
	];

	let script_with_typed_keys = vec![
		RuntimeEvent::WindowResized { cols: 80, rows: 24 },
		RuntimeEvent::Key(Key::char('i')),
		RuntimeEvent::Key(Key::char('a')),
		RuntimeEvent::Key(Key::char('b')),
		RuntimeEvent::Key(Key::char('c')),
		RuntimeEvent::Key(esc),
	];

	let mut via_paste = Editor::new_scratch();
	let _ = via_paste.pump().await;
	run_script(&mut via_paste, script_with_paste).await;

	let mut via_keys = Editor::new_scratch();
	let _ = via_keys.pump().await;
	run_script(&mut via_keys, script_with_typed_keys).await;

	let text_via_paste = via_paste.buffer().with_doc(|doc| doc.content().to_string());
	let text_via_keys = via_keys.buffer().with_doc(|doc| doc.content().to_string());

	assert_eq!(text_via_paste, "abc");
	assert_eq!(text_via_paste, text_via_keys);
	assert_eq!(via_paste.mode(), via_keys.mode());
	assert_eq!(via_paste.statusline_render_plan(), via_keys.statusline_render_plan());
}

#[tokio::test]
async fn test_runtime_event_scripts_converge_for_multiline_input() {
	let esc = Key::new(KeyCode::Esc);
	let enter = Key::new(KeyCode::Enter);

	let script_with_paste = vec![
		RuntimeEvent::WindowResized { cols: 80, rows: 24 },
		RuntimeEvent::Key(Key::char('i')),
		RuntimeEvent::Paste(String::from("a\r\nb")),
		RuntimeEvent::Key(esc),
	];

	let script_with_typed_keys = vec![
		RuntimeEvent::WindowResized { cols: 80, rows: 24 },
		RuntimeEvent::Key(Key::char('i')),
		RuntimeEvent::Key(Key::char('a')),
		RuntimeEvent::Key(enter),
		RuntimeEvent::Key(Key::char('b')),
		RuntimeEvent::Key(esc),
	];

	let mut via_paste = Editor::new_scratch();
	let _ = via_paste.pump().await;
	run_script(&mut via_paste, script_with_paste).await;

	let mut via_keys = Editor::new_scratch();
	let _ = via_keys.pump().await;
	run_script(&mut via_keys, script_with_typed_keys).await;

	let text_via_paste = via_paste.buffer().with_doc(|doc| doc.content().to_string());
	let text_via_keys = via_keys.buffer().with_doc(|doc| doc.content().to_string());

	assert_eq!(text_via_paste, "a\nb");
	assert_eq!(text_via_paste, text_via_keys);
	assert_eq!(via_paste.mode(), via_keys.mode());
	assert_eq!(via_paste.statusline_render_plan(), via_keys.statusline_render_plan());
}

#[tokio::test]
async fn test_runtime_event_scripts_converge_for_command_palette_completion() {
	let mut via_paste = Editor::new_scratch();
	via_paste.handle_window_resize(120, 30);
	assert!(via_paste.open_command_palette());
	let _ = via_paste.pump().await;
	run_script(&mut via_paste, vec![RuntimeEvent::Paste(String::from("set"))]).await;

	let mut via_keys = Editor::new_scratch();
	via_keys.handle_window_resize(120, 30);
	assert!(via_keys.open_command_palette());
	let _ = via_keys.pump().await;
	run_script(
		&mut via_keys,
		vec![
			RuntimeEvent::Key(Key::char('s')),
			RuntimeEvent::Key(Key::char('e')),
			RuntimeEvent::Key(Key::char('t')),
		],
	)
	.await;

	assert_eq!(via_paste.overlay_kind(), via_keys.overlay_kind());
	assert_eq!(via_paste.completion_popup_render_plan(), via_keys.completion_popup_render_plan());
	assert_eq!(via_paste.statusline_render_plan(), via_keys.statusline_render_plan());
}
