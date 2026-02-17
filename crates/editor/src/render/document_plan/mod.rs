//! Focused document render-plan assembly for frontend consumers.

use xeno_primitives::Style;

use super::{RenderLine, RenderSpan};
use crate::Editor;

#[derive(Debug, Clone)]
pub(crate) struct DocumentRenderPlan {
	pub(crate) title: String,
	pub(crate) lines: Vec<RenderLine<'static>>,
}

impl Editor {
	pub(crate) fn focused_document_render_plan(&mut self) -> DocumentRenderPlan {
		let focused = self.focused_view();
		let title = self.focused_document_title();
		let area = self.view_area(focused);

		if area.width < 2 || area.height == 0 {
			return DocumentRenderPlan {
				title,
				lines: vec![plain_line("document viewport too small")],
			};
		}

		let use_block_cursor = matches!(self.derive_cursor_style(), crate::runtime::CursorStyle::Block);
		let lines = self
			.buffer_view_render_plan(focused, area, use_block_cursor, true)
			.map_or_else(|| vec![plain_line("no focused buffer")], |plan| merge_render_lines(plan.gutter, plan.text));

		DocumentRenderPlan { title, lines }
	}
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
mod tests;
