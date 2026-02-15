//! Editor-backed snippet variable resolver for paths, time, and selection.

use chrono::{Datelike, Local, Timelike};

use crate::Editor;
use crate::buffer::{Buffer, ViewId};
use crate::snippet::SnippetVarResolver;

pub(crate) struct EditorSnippetResolver<'a> {
	ed: &'a Editor,
	buffer_id: ViewId,
	now: chrono::DateTime<Local>,
	selection_text: Option<String>,
}

impl<'a> EditorSnippetResolver<'a> {
	#[cfg_attr(not(feature = "lsp"), allow(dead_code))]
	pub(crate) fn new(ed: &'a Editor, buffer_id: ViewId) -> Self {
		Self::new_for_selection(ed, buffer_id, None, Local::now())
	}

	pub(crate) fn new_for_selection(ed: &'a Editor, buffer_id: ViewId, selection_text: Option<String>, now: chrono::DateTime<Local>) -> Self {
		Self {
			ed,
			buffer_id,
			now,
			selection_text,
		}
	}

	fn buffer(&self) -> Option<&Buffer> {
		self.ed.state.core.buffers.get_buffer(self.buffer_id)
	}
}

impl SnippetVarResolver for EditorSnippetResolver<'_> {
	fn resolve_var(&self, name: &str) -> Option<String> {
		let buffer = self.buffer()?;
		let now = &self.now;
		match name {
			"TM_FILEPATH" => buffer.path().map(|path| path.to_string_lossy().to_string()),
			"TM_DIRECTORY" => buffer.path().and_then(|path| path.parent().map(|parent| parent.to_string_lossy().to_string())),
			"TM_FILENAME" => buffer.path().and_then(|path| path.file_name().map(|name| name.to_string_lossy().to_string())),
			"TM_FILENAME_BASE" => buffer.path().and_then(|path| path.file_stem().map(|stem| stem.to_string_lossy().to_string())),
			"CURRENT_YEAR" => Some(format!("{:04}", now.year())),
			"CURRENT_MONTH" => Some(format!("{:02}", now.month())),
			"CURRENT_DATE" => Some(format!("{:02}", now.day())),
			"CURRENT_HOUR" => Some(format!("{:02}", now.hour())),
			"CURRENT_MINUTE" => Some(format!("{:02}", now.minute())),
			"CURRENT_SECOND" => Some(format!("{:02}", now.second())),
			"SELECTION" | "TM_SELECTED_TEXT" => Some(self.selection_text.clone().unwrap_or_else(|| {
				let primary = buffer.selection.primary();
				if primary.is_point() {
					String::new()
				} else {
					buffer.with_doc(|doc| {
						let (from, to) = primary.extent_clamped(doc.content().len_chars());
						doc.content().slice(from..to).to_string()
					})
				}
			})),
			_ => None,
		}
	}
}

#[cfg(test)]
mod tests;
