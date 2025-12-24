use std::path::PathBuf;
use std::time::Duration;

use kitty_test_harness::{
	kitty_send_keys, pause_briefly, require_kitty, run_with_timeout, wait_for_screen_text_clean,
	with_kitty_capture,
};
use termwiz::input::{KeyCode, Modifiers};

const TEST_TIMEOUT: Duration = Duration::from_secs(15);

fn tome_cmd() -> String {
	env!("CARGO_BIN_EXE_tome").to_string()
}

fn tome_cmd_with_file_named(name: &str) -> String {
	format!("{} {}", tome_cmd(), name)
}

fn workspace_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn reset_test_file(name: &str) {
	let path = workspace_dir().join(name);
	let _ = std::fs::remove_file(&path);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

fn parse_u8_ascii(bytes: &[u8]) -> Option<u8> {
	let s = std::str::from_utf8(bytes).ok()?;
	s.parse::<u8>().ok()
}

fn parse_bg_from_sgr_tokens(tokens: &[&[u8]], mut current: Option<Rgb>) -> Option<Rgb> {
	let mut i = 0;
	while i < tokens.len() {
		let tok = tokens[i];
		// ESC[m is equivalent to ESC[0m
		if tok.is_empty() || tok == b"0" || tok == b"49" {
			current = None;
			i += 1;
			continue;
		}

		// Kitty extended SGR: 48:2:r:g:b
		if let Some(rest) = tok.strip_prefix(b"48:2:") {
			let parts: Vec<&[u8]> = rest.split(|b| *b == b':').collect();
			if parts.len() >= 3 {
				let r = parse_u8_ascii(parts[0])?;
				let g = parse_u8_ascii(parts[1])?;
				let b = parse_u8_ascii(parts[2])?;
				current = Some(Rgb(r, g, b));
			}
			i += 1;
			continue;
		}

		// Semicolon SGR: 48;2;r;g;b
		if tok == b"48" && i + 4 < tokens.len() && tokens[i + 1] == b"2" {
			let r = parse_u8_ascii(tokens[i + 2])?;
			let g = parse_u8_ascii(tokens[i + 3])?;
			let b = parse_u8_ascii(tokens[i + 4])?;
			current = Some(Rgb(r, g, b));
			i += 5;
			continue;
		}

		i += 1;
	}

	current
}

fn bg_at_marker(raw: &[u8], marker: &[u8]) -> Option<Rgb> {
	let marker_pos = raw.windows(marker.len()).position(|w| w == marker)?;

	let mut i = 0;
	let mut bg: Option<Rgb> = None;
	while i < marker_pos {
		if raw[i] == 0x1b && i + 1 < marker_pos && raw[i + 1] == b'[' {
			let mut j = i + 2;
			while j < marker_pos && raw[j] != b'm' {
				j += 1;
			}
			if j >= marker_pos {
				break;
			}

			let params = &raw[i + 2..j];
			if params.is_empty() {
				bg = None;
				i = j + 1;
				continue;
			}

			let tokens: Vec<&[u8]> = params.split(|b| *b == b';').collect();
			if let Some(next_bg) = parse_bg_from_sgr_tokens(&tokens, bg) {
				bg = Some(next_bg);
			} else {
				// parse_bg_from_sgr_tokens may return None if it encountered an invalid token
				// but in that case we still want to handle resets.
				if tokens.iter().any(|t| *t == b"0" || *t == b"49") {
					bg = None;
				}
			}

			i = j + 1;
			continue;
		}

		i += 1;
	}

	bg
}

#[serial_test::serial]
#[test]
fn embedded_terminal_background_matches_popup_background_in_kitty_dump() {
	if std::env::var_os("KITTY_TESTS").is_none() {
		return;
	}

	if !require_kitty() {
		return;
	}

	// Default theme is solarized_dark:
	// - UI BG (base03) = 0,43,54
	// - Popup BG (base02) = 7,54,66
	let expected_ui_bg = Rgb(0, 43, 54);
	let expected_popup_bg = Rgb(7, 54, 66);

	let file = "kitty-test-embedded-terminal-colors.txt";
	reset_test_file(file);

	let doc_marker = "__DOC_BG_MARK__";
	let term_marker = "__TERM_BG_MARK__";

	run_with_timeout(TEST_TIMEOUT, move || {
		with_kitty_capture(&workspace_dir(), &tome_cmd_with_file_named(file), |kitty| {
			pause_briefly();

			// Put a marker into the main editor buffer.
			kitty_send_keys!(
				kitty,
				KeyCode::Char('i'),
				KeyCode::Char('_'),
				KeyCode::Char('_'),
				KeyCode::Char('D'),
				KeyCode::Char('O'),
				KeyCode::Char('C'),
				KeyCode::Char('_'),
				KeyCode::Char('B'),
				KeyCode::Char('G'),
				KeyCode::Char('_'),
				KeyCode::Char('M'),
				KeyCode::Char('A'),
				KeyCode::Char('R'),
				KeyCode::Char('K'),
				KeyCode::Char('_'),
				KeyCode::Char('_')
			);
			kitty_send_keys!(kitty, KeyCode::Escape);

			let (_raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(3), |_raw, clean| {
					clean.contains(doc_marker)
				});
			assert!(clean.contains(doc_marker), "clean: {clean:?}");

			// Open the embedded terminal (keeps terminal focused, which disables the editor cursor overlay).
			kitty_send_keys!(kitty, (KeyCode::Char('t'), Modifiers::CTRL));
			pause_briefly();
			pause_briefly();

			// Emit a marker from the shell so we can locate it in the captured raw dump.
			kitty_send_keys!(
				kitty,
				KeyCode::Char('e'),
				KeyCode::Char('c'),
				KeyCode::Char('h'),
				KeyCode::Char('o'),
				KeyCode::Char(' '),
				KeyCode::Char('_'),
				KeyCode::Char('_'),
				KeyCode::Char('T'),
				KeyCode::Char('E'),
				KeyCode::Char('R'),
				KeyCode::Char('M'),
				KeyCode::Char('_'),
				KeyCode::Char('B'),
				KeyCode::Char('G'),
				KeyCode::Char('_'),
				KeyCode::Char('M'),
				KeyCode::Char('A'),
				KeyCode::Char('R'),
				KeyCode::Char('K'),
				KeyCode::Char('_'),
				KeyCode::Char('_'),
				KeyCode::Enter
			);

			let (raw, clean) =
				wait_for_screen_text_clean(kitty, Duration::from_secs(5), |_raw, clean| {
					clean.contains(term_marker)
				});
			assert!(clean.contains(term_marker), "clean: {clean:?}");

			let raw_bytes = raw.as_bytes();
			let doc_bg = bg_at_marker(raw_bytes, doc_marker.as_bytes());
			let term_bg = bg_at_marker(raw_bytes, term_marker.as_bytes());

			assert_eq!(
				doc_bg,
				Some(expected_ui_bg),
				"expected doc marker bg {:?}, got {:?}. raw: {:?}",
				expected_ui_bg,
				doc_bg,
				raw
			);
			assert_eq!(
				term_bg,
				Some(expected_popup_bg),
				"expected terminal marker bg {:?}, got {:?}. raw: {:?}",
				expected_popup_bg,
				term_bg,
				raw
			);
			assert_ne!(doc_bg, term_bg, "terminal bg should differ from doc bg");
		});
	});
}
