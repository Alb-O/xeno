use xeno_primitives::Style;

use super::{RenderLine, RenderSpan};
use crate::{Editor, ViewId};

#[derive(Debug, Clone)]
pub struct DocumentRenderPlan {
	pub title: String,
	pub lines: Vec<RenderLine<'static>>,
}

impl Editor {
	pub fn focused_document_render_plan(&mut self) -> DocumentRenderPlan {
		let focused = self.focused_view();
		let title = focused_document_title(self, focused);
		let area = self.view_area(focused);

		if area.width < 2 || area.height == 0 {
			return DocumentRenderPlan {
				title,
				lines: vec![plain_line("document viewport too small")],
			};
		}

		let lines = self
			.buffer_view_render_plan(focused, area, true, true)
			.map_or_else(|| vec![plain_line("no focused buffer")], |plan| merge_render_lines(plan.gutter, plan.text));

		DocumentRenderPlan { title, lines }
	}
}

fn focused_document_title(editor: &Editor, focused: ViewId) -> String {
	editor
		.get_buffer(focused)
		.and_then(|buffer| buffer.path().as_ref().map(|path| path.display().to_string()))
		.unwrap_or_else(|| String::from("[scratch]"))
}

fn merge_render_lines(gutter: Vec<RenderLine<'static>>, text: Vec<RenderLine<'static>>) -> Vec<RenderLine<'static>> {
	let row_count = gutter.len().max(text.len());
	let mut rows = Vec::with_capacity(row_count);

	for idx in 0..row_count {
		let mut spans = Vec::new();
		if let Some(gutter_line) = gutter.get(idx) {
			spans.extend(gutter_line.spans.iter().cloned());
		}
		if let Some(text_line) = text.get(idx) {
			spans.extend(text_line.spans.iter().cloned());
		}
		rows.push(RenderLine { spans, style: None });
	}

	rows
}

fn plain_line(content: impl Into<String>) -> RenderLine<'static> {
	RenderLine::from(vec![RenderSpan::styled(content.into(), Style::default())])
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn focused_document_render_plan_renders_lines_after_resize() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(80, 24);

		let plan = editor.focused_document_render_plan();
		assert!(!plan.lines.is_empty());
	}

	#[test]
	fn focused_document_render_plan_uses_scratch_title_without_path() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(80, 24);

		let plan = editor.focused_document_render_plan();
		assert_eq!(plan.title, "[scratch]");
	}

	#[test]
	fn focused_document_render_plan_uses_path_title_for_file_buffers() {
		let file = tempfile::NamedTempFile::new().expect("temp file");
		std::fs::write(file.path(), "alpha\n").expect("write file");

		let mut editor = Editor::new_scratch();
		let loader = editor.config().language_loader.clone();
		let _ = editor.buffer_mut().set_path(Some(file.path().to_path_buf()), Some(&loader));
		editor.handle_window_resize(80, 24);

		let plan = editor.focused_document_render_plan();
		assert_eq!(plan.title, file.path().display().to_string());
	}

	#[test]
	fn focused_document_render_plan_returns_placeholder_for_tiny_viewport() {
		let mut editor = Editor::new_scratch();
		editor.handle_window_resize(1, 1);

		let plan = editor.focused_document_render_plan();
		assert_eq!(plan.lines.len(), 1);
		assert_eq!(plan.lines[0].spans[0].content.as_ref(), "document viewport too small");
	}
}
