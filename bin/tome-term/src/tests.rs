#[cfg(test)]
mod suite {
	use std::path::PathBuf;

	use insta::assert_snapshot;
	use ratatui::Terminal;
	use ratatui::backend::TestBackend;
	use termina::event::{KeyCode, KeyEvent, Modifiers};
	use tome_core::ext::{CommandContext, CommandOutcome};
	use tome_core::{Mode, Selection};

	use crate::editor::Editor;
	use crate::theme::{CMD_THEME, THEMES, get_theme};

	fn test_editor(content: &str) -> Editor {
		Editor::from_content(content.to_string(), Some(PathBuf::from("test.txt")))
	}

	#[test]
	fn test_themes_registry() {
		assert!(THEMES.len() >= 5);

		let default = get_theme("default");
		assert!(default.is_some());

		let solarized = get_theme("solarized_dark");
		assert!(solarized.is_some());
		assert_eq!(get_theme("solarized").unwrap().name, "solarized_dark");
		assert_eq!(get_theme("solarized-dark").unwrap().name, "solarized_dark");

		let monokai = get_theme("monokai");
		assert!(monokai.is_some());
		assert_eq!(get_theme("monokai-extended").unwrap().name, "monokai");

		let one_dark = get_theme("one_dark");
		assert!(one_dark.is_some());
		assert_eq!(get_theme("onedark").unwrap().name, "one_dark");
		assert_eq!(get_theme("one").unwrap().name, "one_dark");

		let gruvbox = get_theme("gruvbox");
		assert!(gruvbox.is_some());
		assert_eq!(get_theme("gruvbox-dark").unwrap().name, "gruvbox");
	}

	#[test]
	fn test_theme_command() {
		let mut editor = Editor::new_scratch();

		assert_eq!(editor.theme.name, "solarized_dark");

		let args = ["default"];
		let mut ctx = CommandContext {
			editor: &mut editor,
			args: &args,
			count: 1,
			register: None,
			user_data: CMD_THEME.user_data,
		};

		let result = (CMD_THEME.handler)(&mut ctx);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), CommandOutcome::Ok);

		assert_eq!(editor.theme.name, "default");

		let args_typo = ["solarised"];
		let mut ctx_typo = CommandContext {
			editor: &mut editor,
			args: &args_typo,
			count: 1,
			register: None,
			user_data: CMD_THEME.user_data,
		};

		let result_typo = (CMD_THEME.handler)(&mut ctx_typo);
		assert!(result_typo.is_err());
		if let Err(tome_core::ext::CommandError::Failed(msg)) = result_typo {
			assert!(msg.contains("Did you mean 'solarized_dark'?"));
		} else {
			panic!("Expected Failed error with suggestion");
		}
	}

	#[test]
	fn test_render_empty() {
		let mut editor = test_editor("");
		let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_render_with_content() {
		let mut editor = test_editor("Hello, World!\nThis is a test.\nLine 3.");
		let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_render_tabs_expand_and_cursor_visible() {
		let mut editor = test_editor("\tX");
		let gutter_width = editor.gutter_width();

		let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();

		let buffer = terminal.backend().buffer();
		let x = gutter_width;
		assert_eq!(
			buffer.cell((x, 0)).unwrap().bg,
			editor.theme.colors.ui.cursor_bg,
			"cursor should render even when on a tab"
		);
		assert_eq!(
			buffer.cell((x + 4, 0)).unwrap().symbol(),
			"X",
			"tab should expand to spaces"
		);
	}

	#[test]
	fn test_render_insert_mode() {
		let mut editor = test_editor("Hello");
		editor.input.set_mode(Mode::Insert);
		let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_render_after_typing() {
		let mut editor = test_editor("");
		editor.input.set_mode(Mode::Insert);
		editor.insert_text("abc");
		let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_render_with_selection() {
		let mut editor = test_editor("Hello, World!");
		editor.handle_key(KeyEvent::new(KeyCode::Char('L'), Modifiers::SHIFT));
		editor.handle_key(KeyEvent::new(KeyCode::Char('L'), Modifiers::SHIFT));
		editor.handle_key(KeyEvent::new(KeyCode::Char('L'), Modifiers::SHIFT));
		let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_render_cursor_movement() {
		let mut editor = test_editor("Hello\nWorld");
		editor.handle_key(KeyEvent::new(KeyCode::Char('j'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('l'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('l'), Modifiers::NONE));
		let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_word_movement() {
		let mut editor = test_editor("hello world test");
		editor.handle_key(KeyEvent::new(KeyCode::Char('w'), Modifiers::NONE));
		assert_eq!(editor.cursor, 6);
	}

	#[test]
	fn test_goto_mode() {
		let mut editor = test_editor("line1\nline2\nline3");
		editor.handle_key(KeyEvent::new(KeyCode::Char('g'), Modifiers::NONE));
		assert!(matches!(editor.mode(), Mode::Goto));
		editor.handle_key(KeyEvent::new(KeyCode::Char('g'), Modifiers::NONE));
		assert_eq!(editor.cursor, 0);
	}

	#[test]
	fn test_undo_redo() {
		let mut editor = test_editor("hello");
		assert_eq!(editor.doc.to_string(), "hello");

		editor.handle_key(KeyEvent::new(KeyCode::Char('%'), Modifiers::NONE));
		assert_eq!(editor.selection.primary().min(), 0);
		assert_eq!(editor.selection.primary().max(), 5);

		editor.handle_key(KeyEvent::new(KeyCode::Char('d'), Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "", "after delete");
		assert_eq!(editor.undo_stack.len(), 1, "undo stack should have 1 entry");

		editor.handle_key(KeyEvent::new(KeyCode::Char('u'), Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "hello", "after undo");
		assert_eq!(editor.redo_stack.len(), 1, "redo stack should have 1 entry");
		assert_eq!(editor.undo_stack.len(), 0, "undo stack should be empty");

		editor.handle_key(KeyEvent::new(KeyCode::Char('U'), Modifiers::SHIFT));
		assert_eq!(
			editor.redo_stack.len(),
			0,
			"redo stack should be empty after redo"
		);
		assert_eq!(editor.doc.to_string(), "", "after redo");
	}

	#[test]
	fn test_insert_grouped_undo() {
		let mut editor = test_editor("");

		editor.handle_key(KeyEvent::new(KeyCode::Char('i'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('a'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('b'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('c'), Modifiers::NONE));

		editor.handle_key(KeyEvent::new(KeyCode::Escape, Modifiers::NONE));

		assert_eq!(editor.doc.to_string(), "abc");
		assert_eq!(
			editor.undo_stack.len(),
			1,
			"single undo entry for insert session"
		);

		editor.handle_key(KeyEvent::new(KeyCode::Char('u'), Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "");
		assert_eq!(editor.redo_stack.len(), 1);
	}

	#[test]
	fn test_insert_newline_single_cursor() {
		let mut editor = test_editor("");

		editor.handle_key(KeyEvent::new(KeyCode::Char('i'), Modifiers::NONE));
		assert!(matches!(editor.mode(), Mode::Insert));

		editor.handle_key(KeyEvent::new(KeyCode::Enter, Modifiers::NONE));

		assert_eq!(editor.doc.len_lines(), 2, "should have 2 lines after Enter");
		assert_eq!(editor.cursor, 1, "cursor should be at position 1");
		assert_eq!(
			editor.cursor_line(),
			1,
			"cursor should be on line 1 (second line)"
		);

		let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();

		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_insert_mode_arrow_keys() {
		let mut editor = test_editor("hello world");
		assert_eq!(editor.cursor, 0, "start at position 0");

		editor.handle_key(KeyEvent::new(KeyCode::Char('i'), Modifiers::NONE));
		assert!(
			matches!(editor.mode(), Mode::Insert),
			"should be in insert mode"
		);

		editor.handle_key(KeyEvent::new(KeyCode::Right, Modifiers::NONE));
		assert_eq!(editor.cursor, 1, "after Right arrow, cursor at 1");

		editor.handle_key(KeyEvent::new(KeyCode::Right, Modifiers::NONE));
		assert_eq!(editor.cursor, 2, "after Right arrow, cursor at 2");

		editor.handle_key(KeyEvent::new(KeyCode::Left, Modifiers::NONE));
		assert_eq!(editor.cursor, 1, "after Left arrow, cursor at 1");

		editor.handle_key(KeyEvent::new(KeyCode::Down, Modifiers::NONE));
		assert!(
			matches!(editor.mode(), Mode::Insert),
			"still in insert mode after arrows"
		);
	}

	#[test]
	fn test_soft_wrap_long_line() {
		let long_line = "The quick brown fox jumps over the lazy dog and keeps on running";
		let mut editor = test_editor(long_line);

		let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_soft_wrap_word_boundary() {
		let text = "hello world this is a test of word wrapping behavior";
		let mut editor = test_editor(text);

		let mut terminal = Terminal::new(TestBackend::new(30, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_line_numbers_multiple_lines() {
		let text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
		let mut editor = test_editor(text);

		let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}

	#[test]
	fn test_backspace_deletes_backwards() {
		let mut editor = test_editor("hello");

		editor.handle_key(KeyEvent::new(KeyCode::Char('a'), Modifiers::NONE));
		assert!(matches!(editor.mode(), Mode::Insert));
		assert_eq!(editor.cursor, 1, "cursor at 1 after 'a'");

		editor.handle_key(KeyEvent::new(KeyCode::Backspace, Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "ello", "first char deleted");
		assert_eq!(editor.cursor, 0, "cursor moved back to 0");

		editor.handle_key(KeyEvent::new(KeyCode::Backspace, Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "ello", "no change when at start");
		assert_eq!(editor.cursor, 0, "cursor stays at 0");
	}

	#[test]
	fn test_toggle_terminal_panel_changes_layout() {
		let text = (1..=20)
			.map(|i| format!("Line {}", i))
			.collect::<Vec<_>>()
			.join("\n");
		let mut editor = test_editor(&text);

		let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		let buffer = terminal.backend().buffer();
		assert_eq!(
			buffer.cell((2, 8)).unwrap().symbol(),
			"9",
			"when terminal is closed, document should reach the last row"
		);

		editor
			.ui
			.toggle_panel(crate::ui::panels::terminal::TERMINAL_PANEL_ID);
		assert!(
			editor
				.ui
				.dock
				.is_open(crate::ui::panels::terminal::TERMINAL_PANEL_ID),
			"Ctrl+t should open terminal panel"
		);
		terminal.draw(|frame| editor.render(frame)).unwrap();
		let buffer = terminal.backend().buffer();
		assert_ne!(
			buffer.cell((2, 8)).unwrap().symbol(),
			"9",
			"when terminal is open, document height should shrink"
		);

		editor
			.ui
			.toggle_panel(crate::ui::panels::terminal::TERMINAL_PANEL_ID);
		assert!(
			!editor
				.ui
				.dock
				.is_open(crate::ui::panels::terminal::TERMINAL_PANEL_ID),
			"Ctrl+t should close terminal panel"
		);
		terminal.draw(|frame| editor.render(frame)).unwrap();
		let buffer = terminal.backend().buffer();
		assert_eq!(
			buffer.cell((2, 8)).unwrap().symbol(),
			"9",
			"after closing terminal, document should fill the area again"
		);
	}

	#[test]
	fn test_scroll_down_when_cursor_at_bottom() {
		let text = (1..=20)
			.map(|i| format!("Line {}", i))
			.collect::<Vec<_>>()
			.join("\n");
		let mut editor = test_editor(&text);

		let viewport_height = 8;

		assert_eq!(editor.scroll_line, 0, "starts at top");
		assert_eq!(editor.cursor_line(), 0, "cursor on line 0");

		for _ in 0..10 {
			editor.handle_key(KeyEvent::new(KeyCode::Char('j'), Modifiers::NONE));
		}

		assert_eq!(editor.cursor_line(), 10, "cursor on line 10");

		let mut terminal = Terminal::new(TestBackend::new(40, viewport_height as u16 + 1)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();

		assert_eq!(
			editor.scroll_line, 3,
			"should scroll down one line so cursor can fit in viewport"
		);
		assert_eq!(editor.scroll_segment, 0, "no wrapping in this test");

		assert!(
			editor.scroll_line + viewport_height > editor.cursor_line(),
			"cursor line {} should be visible in viewport (scroll_line={}, height={})",
			editor.cursor_line(),
			editor.scroll_line,
			viewport_height
		);
	}

	#[test]
	fn test_scroll_with_soft_wrapped_lines() {
		let long_line = "This is a very long line that will wrap multiple times in the viewport";
		let text = format!("{}\n{}\n{}\nshort\nshort", long_line, long_line, long_line);
		let mut editor = test_editor(&text);

		let mut terminal = Terminal::new(TestBackend::new(20, 6)).unwrap();

		for _ in 0..4 {
			editor.handle_key(KeyEvent::new(KeyCode::Char('j'), Modifiers::NONE));
		}

		assert_eq!(editor.cursor_line(), 4, "cursor on line 4 (last 'short')");

		terminal.draw(|frame| editor.render(frame)).unwrap();

		assert!(
			editor.scroll_line > 0 || editor.scroll_segment > 0,
			"scroll should advance to show cursor through wrapped lines, got scroll_line={}, scroll_segment={}",
			editor.scroll_line,
			editor.scroll_segment
		);
	}

	#[test]
	fn test_visual_vertical_movement_in_wrapped_line() {
		let long_line = "The quick brown fox jumps over the lazy dog and keeps running";
		let mut editor = test_editor(long_line);

		editor.text_width = 20;

		assert_eq!(editor.cursor, 0, "starts at 0");

		editor.handle_key(KeyEvent::new(KeyCode::Char('j'), Modifiers::NONE));

		let head = editor.cursor;
		assert!(
			head > 0 && head < long_line.len(),
			"cursor should move within wrapped line segments, got head={}",
			head
		);
		assert_eq!(editor.cursor_line(), 0, "should still be on doc line 0");

		editor.handle_key(KeyEvent::new(KeyCode::Char('k'), Modifiers::NONE));

		assert_eq!(editor.cursor, 0, "should return to start");
	}

	#[test]
	fn test_visual_movement_across_doc_lines() {
		let text = "short\nanother short";
		let mut editor = test_editor(text);
		editor.text_width = 80;

		assert_eq!(editor.cursor_line(), 0);

		editor.handle_key(KeyEvent::new(KeyCode::Char('j'), Modifiers::NONE));

		assert_eq!(editor.cursor_line(), 1, "should move to next doc line");

		editor.handle_key(KeyEvent::new(KeyCode::Char('k'), Modifiers::NONE));

		assert_eq!(editor.cursor_line(), 0, "should return to first doc line");
	}

	#[test]
	fn test_wrap_preserves_leading_spaces() {
		let mut editor = test_editor("hello");
		editor.text_width = 10;

		let segments = editor.wrap_line("hello     world", 10);
		assert_eq!(segments.len(), 2);
		assert_eq!(segments[0].text, "hello     ");
		assert_eq!(segments[0].start_offset, 0);
		assert_eq!(segments[1].text, "world");
		assert_eq!(segments[1].start_offset, 10);

		let total_chars: usize = segments.iter().map(|s| s.text.chars().count()).sum();
		assert_eq!(total_chars, 15, "all characters preserved");

		let segments2 = editor.wrap_line("hello    world test", 11);
		let total2: usize = segments2.iter().map(|s| s.text.chars().count()).sum();
		assert_eq!(total2, 19, "all characters preserved");
	}

	#[test]
	fn test_shift_end_extends_selection() {
		let mut editor = test_editor("hello world");
		assert_eq!(editor.cursor, 0);
		assert_eq!(editor.selection.primary().anchor, 0);

		editor.handle_key(KeyEvent::new(KeyCode::End, Modifiers::SHIFT));

		let sel = editor.selection.primary();
		assert_eq!(sel.anchor, 0, "anchor should stay at start");
		assert_eq!(sel.head, 11, "head should move to end of line");
	}

	#[test]
	fn test_shift_home_extends_selection() {
		let mut editor = test_editor("hello world");

		editor.handle_key(KeyEvent::new(KeyCode::End, Modifiers::SHIFT));
		let sel_after_end = editor.selection.primary();
		assert_eq!(sel_after_end.anchor, 0, "anchor stays at start");
		assert_eq!(sel_after_end.head, 11, "head moves to end");

		editor.handle_key(KeyEvent::new(KeyCode::Home, Modifiers::SHIFT));

		let sel = editor.selection.primary();
		assert_eq!(sel.head, 0, "head should move to start");
		assert_eq!(sel.anchor, 0, "anchor stays at original position");
	}

	#[test]
	fn test_shift_right_extends_selection() {
		let mut editor = test_editor("hello");
		assert_eq!(editor.cursor, 0);

		editor.handle_key(KeyEvent::new(KeyCode::Right, Modifiers::SHIFT));
		editor.handle_key(KeyEvent::new(KeyCode::Right, Modifiers::SHIFT));
		editor.handle_key(KeyEvent::new(KeyCode::Right, Modifiers::SHIFT));

		let sel = editor.selection.primary();
		assert_eq!(sel.anchor, 0, "anchor should stay at start");
		assert_eq!(sel.head, 3, "head should move 3 positions");
	}

	#[test]
	fn test_split_lines_via_keybindings() {
		let mut editor = test_editor("one\ntwo\nthree\n");

		editor.handle_key(KeyEvent::new(KeyCode::Char('%'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('s'), Modifiers::ALT));

		let ranges = editor.selection.ranges();
		assert_eq!(ranges.len(), 3, "expected per-line selections after split");

		let line_ends: Vec<usize> = (0..editor.doc.len_lines())
			.map(|line| {
				if line + 1 < editor.doc.len_lines() {
					editor.doc.line_to_char(line + 1)
				} else {
					editor.doc.len_chars()
				}
			})
			.collect();

		assert_eq!(ranges[0].min(), 0);
		assert_eq!(ranges[0].max(), line_ends[0]);
		assert_eq!(ranges[1].min(), line_ends[0]);
		assert_eq!(ranges[1].max(), line_ends[1]);
		assert_eq!(ranges[2].min(), line_ends[1]);
		assert_eq!(ranges[2].max(), line_ends[2]);

		assert_eq!(editor.doc.to_string(), "one\ntwo\nthree\n");
	}

	#[test]
	fn test_duplicate_down_then_delete() {
		let mut editor = test_editor("alpha\nbeta\ngamma\n");

		editor.cursor = editor.doc.line_to_char(1);
		editor.selection = Selection::point(editor.cursor);

		editor.handle_key(KeyEvent::new(KeyCode::Char('x'), Modifiers::NONE));
		assert_eq!(
			editor.selection.ranges().len(),
			1,
			"expected single line selection after x"
		);

		editor.handle_key(KeyEvent::new(KeyCode::Char('+'), Modifiers::NONE));
		assert_eq!(
			editor.selection.ranges().len(),
			2,
			"expected selection duplicated down"
		);

		editor.handle_key(KeyEvent::new(KeyCode::Char('d'), Modifiers::NONE));

		let text = editor.doc.to_string();
		assert!(text.contains("alpha"));
		assert!(!text.contains("beta"), "doc after delete: {text:?}");
		assert!(!text.contains("gamma"), "doc after delete: {text:?}");
	}

	#[test]
	fn test_insert_across_multiple_selections() {
		let mut editor = test_editor("one\ntwo\nthree\n");

		editor.handle_key(KeyEvent::new(KeyCode::Char('%'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('s'), Modifiers::ALT));

		editor.insert_text("X");

		assert_eq!(editor.doc.to_string(), "Xone\nXtwo\nXthree\n");
	}

	#[test]
	fn test_duplicate_down_insert_inserts_at_all_cursors() {
		let mut editor = test_editor("one\ntwo\nthree\n");

		editor.handle_key(KeyEvent::new(KeyCode::Char('C'), Modifiers::SHIFT));
		editor.handle_key(KeyEvent::new(KeyCode::Char('C'), Modifiers::SHIFT));

		editor.handle_key(KeyEvent::new(KeyCode::Char('i'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('a'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('b'), Modifiers::NONE));

		assert_eq!(editor.doc.to_string(), "abone\nabtwo\nabthree\n");

		editor.handle_key(KeyEvent::new(KeyCode::Backspace, Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "aone\natwo\nathree\n");

		editor.handle_key(KeyEvent::new(KeyCode::Backspace, Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "one\ntwo\nthree\n");

		editor.handle_key(KeyEvent::new(KeyCode::End, Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('X'), Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "oneX\ntwoX\nthreeX\n");
	}
}
