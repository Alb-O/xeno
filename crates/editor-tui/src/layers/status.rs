use xeno_editor::Editor;
use xeno_editor::render_api::{StatuslineRenderSegment, StatuslineRenderStyle};
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::{Block, Paragraph};

fn segment_to_span(ed: &Editor, segment: &StatuslineRenderSegment) -> Span<'static> {
	let mut style = ed.statusline_segment_style(segment.style());
	if matches!(segment.style(), StatuslineRenderStyle::Mode) {
		style = style.add_modifier(Modifier::BOLD);
	}
	Span::styled(segment.text().to_string(), style)
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame, area: Rect) {
	let status_bg = Block::default().style(Style::default().bg(ed.config().theme.colors.ui.bg));
	frame.render_widget(status_bg, area);

	let spans: Vec<_> = ed.statusline_render_plan().iter().map(|segment| segment_to_span(ed, segment)).collect();
	frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
