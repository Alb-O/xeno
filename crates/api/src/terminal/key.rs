//! Terminal key input handling.

use evildoer_manifest::{SplitKey, SplitKeyCode, SplitModifiers};

/// Converts a SplitKey to terminal escape sequence bytes.
pub fn key_to_bytes(key: &SplitKey) -> Vec<u8> {
	match key.code {
		SplitKeyCode::Char(c) => {
			if key.modifiers.contains(SplitModifiers::CTRL) {
				let byte = c.to_ascii_lowercase() as u8;
				if byte.is_ascii_lowercase() {
					vec![byte - b'a' + 1]
				} else {
					vec![byte]
				}
			} else {
				let mut b = [0; 4];
				c.encode_utf8(&mut b).as_bytes().to_vec()
			}
		}
		SplitKeyCode::Enter => vec![b'\r'],
		SplitKeyCode::Backspace => vec![0x7f],
		SplitKeyCode::Tab => vec![b'\t'],
		SplitKeyCode::Up => b"\x1b[A".to_vec(),
		SplitKeyCode::Down => b"\x1b[B".to_vec(),
		SplitKeyCode::Right => b"\x1b[C".to_vec(),
		SplitKeyCode::Left => b"\x1b[D".to_vec(),
		SplitKeyCode::Home => b"\x1b[H".to_vec(),
		SplitKeyCode::End => b"\x1b[F".to_vec(),
		SplitKeyCode::PageUp => b"\x1b[5~".to_vec(),
		SplitKeyCode::PageDown => b"\x1b[6~".to_vec(),
		SplitKeyCode::Delete => b"\x1b[3~".to_vec(),
		SplitKeyCode::Insert => b"\x1b[2~".to_vec(),
		SplitKeyCode::F(n) => match n {
			1 => b"\x1bOP".to_vec(),
			2 => b"\x1bOQ".to_vec(),
			3 => b"\x1bOR".to_vec(),
			4 => b"\x1bOS".to_vec(),
			5 => b"\x1b[15~".to_vec(),
			6 => b"\x1b[17~".to_vec(),
			7 => b"\x1b[18~".to_vec(),
			8 => b"\x1b[19~".to_vec(),
			9 => b"\x1b[20~".to_vec(),
			10 => b"\x1b[21~".to_vec(),
			11 => b"\x1b[23~".to_vec(),
			12 => b"\x1b[24~".to_vec(),
			_ => vec![],
		},
		SplitKeyCode::Escape => vec![0x1b],
	}
}
