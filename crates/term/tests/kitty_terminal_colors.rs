use std::path::PathBuf;
use std::time::Duration;

#[allow(unused_imports)]
use kitty_test_harness::{
	kitty_send_keys, pause_briefly, require_kitty, run_with_timeout, wait_for_screen_text_clean,
	with_kitty_capture,
};
#[allow(unused_imports)]
use termwiz::input::{KeyCode, Modifiers};

#[allow(dead_code)]
const TEST_TIMEOUT: Duration = Duration::from_secs(15);

#[allow(dead_code)]
fn tome_cmd() -> String {
	env!("CARGO_BIN_EXE_tome").to_string()
}

#[allow(dead_code)]
fn tome_cmd_with_file_named(name: &str) -> String {
	format!("{} {}", tome_cmd(), name)
}

#[allow(dead_code)]
fn workspace_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[allow(dead_code)]
fn reset_test_file(name: &str) {
	let path = workspace_dir().join(name);
	let _ = std::fs::remove_file(&path);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

#[allow(dead_code)]
fn parse_u8_ascii(bytes: &[u8]) -> Option<u8> {
	let s = std::str::from_utf8(bytes).ok()?;
	s.parse::<u8>().ok()
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
