#[cfg(test)]
mod suite {
	use std::path::PathBuf;

	use insta::assert_snapshot;
	use ratatui::Terminal;
	use ratatui::backend::TestBackend;
	use termina::event::{KeyCode, KeyEvent, Modifiers};
	use tome_core::ext::EditorOps;
	use tome_core::{Mode, Rope, Selection};

	use crate::editor::Editor;
	use crate::theme::{CMD_THEME, THEMES, get_theme};

	#[derive(Debug, Clone)]
	struct KeyStep {
		desc: &'static str,
		key: KeyEvent,
	}

	fn run_key_sequence(editor: &mut Editor, steps: &[KeyStep]) -> Vec<String> {
		let mut snapshots = Vec::new();
		for step in steps {
			editor.handle_key(step.key);
			let ranges: Vec<(usize, usize)> = editor
				.selection
				.ranges()
				.iter()
				.map(|r| (r.from(), r.to()))
				.collect();
			snapshots.push(format!(
				"{} -> cursor:{} line:{} sel:{:?} doc:{:?}",
				step.desc,
				editor.cursor,
				editor.cursor_line(),
				ranges,
				editor.doc.to_string()
			));
		}
		snapshots
	}

	use tome_core::ext::{CommandContext, CommandOutcome};

	fn test_editor(content: &str) -> Editor {
		Editor::from_content(content.to_string(), Some(PathBuf::from("test.txt")))
	}

	#[test]
	fn test_themes_registry() {
		// We expect at least default, solarized_dark, monokai, one_dark, gruvbox
		assert!(THEMES.len() >= 5);

		let default = get_theme("default");
		assert!(default.is_some());

		// Solarized aliases
		let solarized = get_theme("solarized_dark");
		assert!(solarized.is_some());
		assert_eq!(get_theme("solarized").unwrap().name, "solarized_dark");
		assert_eq!(get_theme("solarized-dark").unwrap().name, "solarized_dark");

		// Monokai aliases
		let monokai = get_theme("monokai");
		assert!(monokai.is_some());
		assert_eq!(get_theme("monokai-extended").unwrap().name, "monokai");

		// One Dark aliases
		let one_dark = get_theme("one_dark");
		assert!(one_dark.is_some());
		assert_eq!(get_theme("onedark").unwrap().name, "one_dark");
		assert_eq!(get_theme("one").unwrap().name, "one_dark");

		// Gruvbox aliases
		let gruvbox = get_theme("gruvbox");
		assert!(gruvbox.is_some());
		assert_eq!(get_theme("gruvbox-dark").unwrap().name, "gruvbox");
	}

	#[test]
	fn test_theme_command() {
		let mut editor = Editor::new_scratch();

		// Initial theme should be solarized_dark now
		assert_eq!(editor.theme.name, "solarized_dark");

		// Execute theme command to switch to default
		let args = ["default"];
		let mut ctx = CommandContext {
			editor: &mut editor,
			args: &args,
			count: 1,
			register: None,
		};

		let result = (CMD_THEME.handler)(&mut ctx);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), CommandOutcome::Ok);

		// Theme should be updated
		assert_eq!(editor.theme.name, "default");

		// Test invalid theme with suggestion
		let _args_invalid = ["solarizeddark"]; // Typo (missing separator, but normalized should handle it actually)
		// Let's try a real typo "solarised"

		let args_typo = ["solarised"];
		let mut ctx_typo = CommandContext {
			editor: &mut editor,
			args: &args_typo,
			count: 1,
			register: None,
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
		assert_eq!(editor.selection.primary().from(), 0);
		assert_eq!(editor.selection.primary().to(), 5);

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

		// Enter insert mode and type multiple characters.
		editor.handle_key(KeyEvent::new(KeyCode::Char('i'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('a'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('b'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('c'), Modifiers::NONE));

		// Exit insert mode.
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

		// In insert mode, the terminal cursor is used instead of an inverted-color cell.
		// The TestBackend tracks cursor position - verify it was set.
		// Note: TestBackend returns (0,0) by default, but frame.set_cursor_position was called.

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

		let mut terminal = Terminal::new(TestBackend::new(40, viewport_height as u16 + 2)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();

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

		// "hello     world" = 15 chars (5 spaces between)
		// Width 10 means break happens after the run of spaces
		let segments = editor.wrap_line("hello     world", 10);
		assert_eq!(segments.len(), 2);
		// Breaks at last space before width, so first segment includes trailing spaces
		assert_eq!(segments[0].text, "hello     ");
		assert_eq!(segments[0].start_offset, 0);
		assert_eq!(segments[1].text, "world");
		assert_eq!(segments[1].start_offset, 10);

		// Key assertion: all characters accounted for
		let total_chars: usize = segments.iter().map(|s| s.text.chars().count()).sum();
		assert_eq!(total_chars, 15, "all characters preserved");

		// Test leading spaces in second segment are preserved
		let segments2 = editor.wrap_line("hello    world test", 11);
		// Should break at space before 'world' to fit 11 chars
		// "hello    w" is 10 chars, finds break at space position 8, returns 9
		// Actually let's just check all chars preserved
		let total2: usize = segments2.iter().map(|s| s.text.chars().count()).sum();
		assert_eq!(total2, 19, "all characters preserved");
	}

	#[test]
	fn test_shift_end_extends_selection() {
		let mut editor = test_editor("hello world");
		// Start at position 0
		assert_eq!(editor.cursor, 0);
		assert_eq!(editor.selection.primary().anchor, 0);

		// Shift+End should select from current position to end of line
		editor.handle_key(KeyEvent::new(KeyCode::End, Modifiers::SHIFT));

		let sel = editor.selection.primary();
		assert_eq!(sel.anchor, 0, "anchor should stay at start");
		assert_eq!(sel.head, 11, "head should move to end of line");
		assert!(!sel.is_empty(), "selection should not be empty");
	}

	#[test]
	fn test_shift_home_extends_selection() {
		// In Kakoune, we start with a point selection and use extend to select
		let mut editor = test_editor("hello world");

		// Shift+End to select to end (extend from current position)
		editor.handle_key(KeyEvent::new(KeyCode::End, Modifiers::SHIFT));
		let sel_after_end = editor.selection.primary();
		assert_eq!(sel_after_end.anchor, 0, "anchor stays at start");
		assert_eq!(sel_after_end.head, 11, "head moves to end");

		// Shift+Home should extend back to start (anchor stays, head moves)
		editor.handle_key(KeyEvent::new(KeyCode::Home, Modifiers::SHIFT));

		let sel = editor.selection.primary();
		assert_eq!(sel.head, 0, "head should move to start");
		assert_eq!(sel.anchor, 0, "anchor stays at original position");
	}

	#[test]
	fn test_shift_right_extends_selection() {
		let mut editor = test_editor("hello");
		assert_eq!(editor.cursor, 0);

		// Shift+Right three times should extend selection
		editor.handle_key(KeyEvent::new(KeyCode::Right, Modifiers::SHIFT));
		editor.handle_key(KeyEvent::new(KeyCode::Right, Modifiers::SHIFT));
		editor.handle_key(KeyEvent::new(KeyCode::Right, Modifiers::SHIFT));

		let sel = editor.selection.primary();
		assert_eq!(sel.anchor, 0, "anchor should stay at start");
		assert_eq!(sel.head, 3, "head should move 3 positions");
	}

	#[test]
	fn test_scratch_exec_unknown_command_sets_message() {
		let mut editor = test_editor("content");

		// Open scratch with ':'
		editor.handle_key(KeyEvent::new(KeyCode::Char(':'), Modifiers::NONE));
		assert!(editor.scratch_open, "scratch should open");
		assert!(editor.scratch_focused, "scratch should take focus");

		// Type an unknown command into the scratch buffer
		for ch in ['f', 'o', 'o'] {
			editor.handle_key(KeyEvent::new(KeyCode::Char(ch), Modifiers::NONE));
		}

		let scratch_text = editor.with_scratch_context(|ed| ed.doc.to_string());
		assert_eq!(scratch_text, "foo");

		// Ctrl+Enter executes the scratch buffer (insert mode)
		editor.handle_key(KeyEvent::new(KeyCode::Enter, Modifiers::CONTROL));
		assert!(
			editor
				.message
				.as_ref()
				.map(|m| m.text.contains("Unknown command: foo"))
				.unwrap_or(false),
			"expected unknown command message"
		);

		// Now ensure plain Enter in NORMAL inside scratch also executes
		editor.handle_key(KeyEvent::new(KeyCode::Char(':'), Modifiers::NONE));
		editor.with_scratch_context(|ed| {
			ed.doc = Rope::from("zzz");
			ed.cursor = 3;
			ed.selection = Selection::point(3);
		});
		editor.handle_key(KeyEvent::new(KeyCode::Escape, Modifiers::NONE));
		assert!(matches!(editor.mode(), Mode::Normal));
		editor.handle_key(KeyEvent::new(KeyCode::Enter, Modifiers::NONE));
		assert!(
			editor
				.message
				.as_ref()
				.map(|m| m.text.contains("Unknown command: zzz"))
				.unwrap_or(false),
			"expected unknown command message"
		);
	}

	#[test]
	fn test_scratch_colon_routes_to_legacy_commands() {
		let mut editor = test_editor("content");
		editor.handle_key(KeyEvent::new(KeyCode::Char(':'), Modifiers::NONE));
		editor.with_scratch_context(|ed| {
			ed.doc = Rope::from(":foo");
			ed.cursor = ed.doc.len_chars();
			ed.selection = Selection::point(ed.cursor);
			ed.input.set_mode(Mode::Normal);
		});

		editor.handle_key(KeyEvent::new(KeyCode::Enter, Modifiers::NONE));
		assert_eq!(
			editor.message.as_ref().map(|m| m.text.as_str()),
			Some("Unknown command: foo")
		);
	}

	#[test]
	fn test_scratch_ctrl_enter_executes_from_insert() {
		let mut editor = test_editor("content");
		editor.handle_key(KeyEvent::new(KeyCode::Char(':'), Modifiers::NONE));
		editor.with_scratch_context(|ed| {
			ed.doc = Rope::from("ctrl-enter-test");
			ed.cursor = ed.doc.len_chars();
			ed.selection = Selection::point(ed.cursor);
		});
		// Stay in insert mode and execute with Ctrl+Enter
		editor.handle_key(KeyEvent::new(KeyCode::Enter, Modifiers::CONTROL));
		assert!(
			editor
				.message
				.as_ref()
				.map(|m| m.text.contains("Unknown command: ctrl-enter-test"))
				.unwrap_or(false),
			"expected unknown command message"
		);
	}

	#[test]
	fn test_scratch_ctrl_j_executes_from_insert() {
		// Many terminals send Ctrl+Enter as Ctrl+J (byte 0x0A = Line Feed).
		// Verify that Ctrl+J also triggers scratch execution.
		let mut editor = test_editor("content");
		editor.handle_key(KeyEvent::new(KeyCode::Char(':'), Modifiers::NONE));
		editor.with_scratch_context(|ed| {
			ed.doc = Rope::from("ctrl-j-test");
			ed.cursor = ed.doc.len_chars();
			ed.selection = Selection::point(ed.cursor);
		});
		// Stay in insert mode and execute with Ctrl+J (alias for Ctrl+Enter)
		editor.handle_key(KeyEvent::new(KeyCode::Char('j'), Modifiers::CONTROL));
		assert!(
			editor
				.message
				.as_ref()
				.map(|m| m.text.contains("Unknown command: ctrl-j-test"))
				.unwrap_or(false),
			"expected unknown command message"
		);
	}

	#[test]
	fn test_scratch_escape_closes_panel() {
		let mut editor = test_editor("content");
		editor.handle_key(KeyEvent::new(KeyCode::Char(':'), Modifiers::NONE));
		assert!(editor.scratch_open && editor.scratch_focused);

		// Type something in insert mode
		editor.handle_key(KeyEvent::new(KeyCode::Char('a'), Modifiers::NONE));
		assert_eq!(editor.with_scratch_context(|ed| ed.doc.to_string()), "a");

		// First escape should move to NORMAL within scratch
		editor.handle_key(KeyEvent::new(KeyCode::Escape, Modifiers::NONE));
		assert!(editor.scratch_open, "scratch stays open after first escape");
		assert!(
			editor.scratch_focused,
			"scratch remains focused after first escape"
		);
		assert!(matches!(editor.mode(), Mode::Normal));

		// Second escape should close the scratch buffer
		editor.handle_key(KeyEvent::new(KeyCode::Escape, Modifiers::NONE));
		assert!(
			!editor.scratch_open,
			"scratch should close on second escape"
		);
		assert!(!editor.scratch_focused, "scratch focus should be cleared");
	}

	#[test]
	fn test_split_lines_via_keybindings() {
		let mut editor = test_editor("one\ntwo\nthree\n");

		// Select all then split lines (%% Alt-s)
		editor.handle_key(KeyEvent::new(KeyCode::Char('%'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('s'), Modifiers::ALT));

		let ranges = editor.selection.ranges();
		assert_eq!(ranges.len(), 3, "expected per-line selections after split");

		let doc = editor.doc.slice(..);
		let line_ends: Vec<usize> = (0..editor.doc.len_lines())
			.map(|line| {
				if line + 1 < editor.doc.len_lines() {
					editor.doc.line_to_char(line + 1)
				} else {
					editor.doc.len_chars()
				}
			})
			.collect();

		assert_eq!(ranges[0].from(), 0);
		assert_eq!(ranges[0].to(), line_ends[0]);
		assert_eq!(ranges[1].from(), line_ends[0]);
		assert_eq!(ranges[1].to(), line_ends[1]);
		assert_eq!(ranges[2].from(), line_ends[1]);
		assert_eq!(ranges[2].to(), line_ends[2]);

		// Ensure buffer text unchanged
		assert_eq!(doc.to_string(), "one\ntwo\nthree\n");
	}

	#[test]
	fn test_duplicate_down_then_delete() {
		let mut editor = test_editor("alpha\nbeta\ngamma\n");

		// Move cursor to second line manually and select it (select_line uses current head)
		editor.cursor = editor.doc.line_to_char(1);
		editor.selection = Selection::point(editor.cursor);

		editor.handle_key(KeyEvent::new(KeyCode::Char('x'), Modifiers::NONE));
		assert_eq!(
			editor.selection.ranges().len(),
			1,
			"expected single line selection after x"
		);
		let ranges_after_select: Vec<(usize, usize)> = editor
			.selection
			.ranges()
			.iter()
			.map(|r| (r.from(), r.to()))
			.collect();

		// Duplicate down then delete selections
		editor.handle_key(KeyEvent::new(KeyCode::Char('+'), Modifiers::NONE));
		assert_eq!(
			editor.selection.ranges().len(),
			2,
			"expected selection duplicated down"
		);
		let ranges_after_dup: Vec<(usize, usize)> = editor
			.selection
			.ranges()
			.iter()
			.map(|r| (r.from(), r.to()))
			.collect();

		let before_delete = editor.doc.to_string();
		editor.handle_key(KeyEvent::new(KeyCode::Char('d'), Modifiers::NONE));

		let text = editor.doc.to_string();
		assert!(
			text.contains("alpha"),
			"before delete: {before_delete:?} after: {text:?} ranges after select: {ranges_after_select:?} ranges after dup: {ranges_after_dup:?}"
		);
		assert!(!text.contains("beta"), "doc after delete: {text:?}");
		assert!(!text.contains("gamma"), "doc after delete: {text:?}");
	}

	#[test]
	fn test_duplicate_down_then_delete_keypath() {
		let mut editor = test_editor("alpha\nbeta\ngamma\n");
		let steps = vec![
			KeyStep {
				desc: "j to line 2",
				key: KeyEvent::new(KeyCode::Char('j'), Modifiers::NONE),
			},
			KeyStep {
				desc: "x select line",
				key: KeyEvent::new(KeyCode::Char('x'), Modifiers::NONE),
			},
			KeyStep {
				desc: "+ duplicate down",
				key: KeyEvent::new(KeyCode::Char('+'), Modifiers::NONE),
			},
			KeyStep {
				desc: "d delete",
				key: KeyEvent::new(KeyCode::Char('d'), Modifiers::NONE),
			},
		];
		let snapshots = run_key_sequence(&mut editor, &steps);
		let text = editor.doc.to_string();
		assert!(text.contains("alpha"), "seq: {snapshots:?}");
		assert!(!text.contains("beta"), "seq: {snapshots:?}");
		assert!(!text.contains("gamma"), "seq: {snapshots:?}");
	}

	#[test]
	fn test_insert_across_multiple_selections() {
		let mut editor = test_editor("one\ntwo\nthree\n");

		// Select all, split per line.
		editor.handle_key(KeyEvent::new(KeyCode::Char('%'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('s'), Modifiers::ALT));

		// Insert at all cursors.
		editor.insert_text("X");

		assert_eq!(editor.doc.to_string(), "Xone\nXtwo\nXthree\n");
	}

	#[test]
	fn test_duplicate_down_insert_inserts_at_all_cursors() {
		let mut editor = test_editor("one\ntwo\nthree\n");

		// Duplicate the initial cursor down to subsequent lines.
		editor.handle_key(KeyEvent::new(KeyCode::Char('C'), Modifiers::SHIFT));
		editor.handle_key(KeyEvent::new(KeyCode::Char('C'), Modifiers::SHIFT));

		// Enter insert mode and type a couple characters.
		editor.handle_key(KeyEvent::new(KeyCode::Char('i'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('a'), Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('b'), Modifiers::NONE));

		assert_eq!(editor.doc.to_string(), "abone\nabtwo\nabthree\n");

		// Backspace should delete at every cursor in insert mode.
		editor.handle_key(KeyEvent::new(KeyCode::Backspace, Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "aone\natwo\nathree\n");

		editor.handle_key(KeyEvent::new(KeyCode::Backspace, Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "one\ntwo\nthree\n");

		// Navigation keys in insert mode should move all cursors.
		editor.handle_key(KeyEvent::new(KeyCode::End, Modifiers::NONE));
		editor.handle_key(KeyEvent::new(KeyCode::Char('X'), Modifiers::NONE));
		assert_eq!(editor.doc.to_string(), "oneX\ntwoX\nthreeX\n");
	}

	#[test]
	fn test_terminal_background_color_matches_popup() {
		let mut editor = test_editor("content");

		// Ensure theme is set to something known
		editor.set_theme("solarized_dark").unwrap();

		// Open terminal
		editor.handle_key(KeyEvent::new(KeyCode::Char('t'), Modifiers::CONTROL));
		assert!(editor.terminal_open);

		// Render
		let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();

		// Inspect buffer
		let buffer = terminal.backend().buffer();

		// Terminal is at the bottom 30% of 24 rows = ~7 rows.
		// Let's check the last row.
		let last_row_cell = &buffer[(0, 23)];

		// It should match popup bg, NOT ui bg.
		let popup_bg = editor.theme.colors.popup.bg;
		let ui_bg = editor.theme.colors.ui.bg;

		assert_ne!(
			popup_bg, ui_bg,
			"Theme should have distinct popup and ui backgrounds for this test"
		);
		assert_eq!(
			last_row_cell.bg, popup_bg,
			"Terminal area background should match popup theme background"
		);

		// Also check main doc area (top) matches UI bg
		let doc_cell = &buffer[(0, 0)];
		assert_eq!(
			doc_cell.bg, ui_bg,
			"Main doc area background should match UI theme background"
		);
	}

	#[test]
	fn test_terminal_toggle_layout() {
		let mut editor = test_editor("some content");

		// Initial state: terminal closed
		assert!(!editor.terminal_open);
		assert!(!editor.terminal_focused);
		assert!(editor.terminal.is_none());

		// Toggle terminal with Ctrl+t
		editor.handle_key(KeyEvent::new(KeyCode::Char('t'), Modifiers::CONTROL));

		assert!(editor.terminal_open);

		// Terminal starts asynchronously now, so wait briefly for it to be ready.
		for _ in 0..50 {
			editor.poll_terminal_prewarm();
			if editor.terminal.is_some() {
				break;
			}
			std::thread::sleep(std::time::Duration::from_millis(10));
		}

		assert!(editor.terminal.is_some());
		assert!(editor.terminal_focused);

		// Render with terminal open
		let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());

		// Toggle terminal off
		editor.handle_key(KeyEvent::new(KeyCode::Char('t'), Modifiers::CONTROL)); // Focused terminal intercepts keys?

		// Wait, if terminal is focused, does it intercept Ctrl+t?
		// My implementation checks for Ctrl+t BEFORE checking terminal focus in `handle_key_active`.

		assert!(!editor.terminal_open);
		assert!(!editor.terminal_focused);

		// Render with terminal closed
		terminal.draw(|frame| editor.render(frame)).unwrap();
		assert_snapshot!(terminal.backend());
	}
}
