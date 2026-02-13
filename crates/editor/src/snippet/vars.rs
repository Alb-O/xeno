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
mod tests {
	use super::EditorSnippetResolver;
	use crate::Editor;
	use crate::snippet::SnippetVarResolver;

	#[test]
	fn current_time_variables_have_expected_shapes_and_ranges() {
		let editor = Editor::new_scratch();
		let resolver = EditorSnippetResolver::new(&editor, editor.focused_view());

		let year = resolver.resolve_var("CURRENT_YEAR").expect("CURRENT_YEAR should resolve");
		assert_eq!(year.len(), 4);
		assert!(year.chars().all(|ch| ch.is_ascii_digit()));

		let month = resolver.resolve_var("CURRENT_MONTH").expect("CURRENT_MONTH should resolve");
		assert_eq!(month.len(), 2);
		let month_num = month.parse::<u32>().expect("CURRENT_MONTH should be numeric");
		assert!((1..=12).contains(&month_num));

		let date = resolver.resolve_var("CURRENT_DATE").expect("CURRENT_DATE should resolve");
		assert_eq!(date.len(), 2);
		let date_num = date.parse::<u32>().expect("CURRENT_DATE should be numeric");
		assert!((1..=31).contains(&date_num));

		let hour = resolver.resolve_var("CURRENT_HOUR").expect("CURRENT_HOUR should resolve");
		assert_eq!(hour.len(), 2);
		let hour_num = hour.parse::<u32>().expect("CURRENT_HOUR should be numeric");
		assert!(hour_num <= 23);

		let minute = resolver.resolve_var("CURRENT_MINUTE").expect("CURRENT_MINUTE should resolve");
		assert_eq!(minute.len(), 2);
		let minute_num = minute.parse::<u32>().expect("CURRENT_MINUTE should be numeric");
		assert!(minute_num <= 59);

		let second = resolver.resolve_var("CURRENT_SECOND").expect("CURRENT_SECOND should resolve");
		assert_eq!(second.len(), 2);
		let second_num = second.parse::<u32>().expect("CURRENT_SECOND should be numeric");
		assert!(second_num <= 59);
	}
}
