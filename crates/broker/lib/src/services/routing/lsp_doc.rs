#[derive(Debug, Clone)]
pub(super) struct LspDocState {
	pub(super) language_id: Option<String>,
	pub(super) text: String,
	pub(super) version: u32,
	pub(super) open: bool,
}

#[derive(Debug)]
pub(super) enum LspDocAction {
	Open {
		uri: String,
		language_id: String,
		version: u32,
		text: String,
	},
	Change {
		uri: String,
		version: u32,
		text: String,
	},
}

#[derive(Debug, Clone)]
pub(super) struct LspContentChange {
	pub(super) range: Option<LspRange>,
	pub(super) text: String,
}

#[derive(Debug, Clone)]
pub(super) struct LspRange {
	pub(super) start: LspPosition,
	pub(super) end: LspPosition,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LspPosition {
	pub(super) line: u32,
	pub(super) character: u32,
}

pub(super) fn apply_content_changes(base: &str, changes: &[LspContentChange]) -> Option<String> {
	let mut text = base.to_string();
	if changes.is_empty() {
		return Some(text);
	}
	for change in changes {
		match &change.range {
			None => {
				text = change.text.clone();
			}
			Some(range) => {
				let start = lsp_offset(&text, range.start);
				let end = lsp_offset(&text, range.end);
				if start > end || start > text.len() || end > text.len() {
					return None;
				}
				text.replace_range(start..end, &change.text);
			}
		}
	}
	Some(text)
}

fn lsp_offset(text: &str, pos: LspPosition) -> usize {
	let mut line_start = 0usize;
	let mut current_line = 0u32;

	for (i, ch) in text.char_indices() {
		if current_line == pos.line {
			break;
		}
		if ch == '\n' {
			current_line += 1;
			line_start = i + ch.len_utf8();
		}
	}

	if current_line < pos.line {
		return text.len();
	}

	let line_slice = &text[line_start..];
	let line_end_rel = line_slice.find('\n').unwrap_or(line_slice.len());
	let line_end = line_start + line_end_rel;

	let mut utf16_units = 0u32;
	let mut byte = line_start;
	for (i, ch) in line_slice[..line_end_rel].char_indices() {
		let units = ch.len_utf16() as u32;
		if utf16_units + units > pos.character {
			break;
		}
		utf16_units += units;
		byte = line_start + i + ch.len_utf8();
		if utf16_units == pos.character {
			break;
		}
	}

	if utf16_units < pos.character {
		line_end
	} else {
		byte
	}
}
