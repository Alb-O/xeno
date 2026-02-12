use xeno_editor::Editor;
use xeno_editor::ui::{StatuslineRenderSegment, StatuslineRenderStyle};
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::{Block, Paragraph};

fn segment_to_span(ed: &Editor, segment: &StatuslineRenderSegment) -> Span<'static> {
	let colors = &ed.config().theme.colors;
	let style = match segment.style {
		StatuslineRenderStyle::Normal => Style::default().fg(colors.ui.fg),
		StatuslineRenderStyle::Mode => colors.mode_style(&ed.mode()).add_modifier(Modifier::BOLD),
		StatuslineRenderStyle::Inverted => Style::default().add_modifier(Modifier::REVERSED),
		StatuslineRenderStyle::Dim => Style::default().fg(colors.semantic.dim),
		StatuslineRenderStyle::Warning => Style::default().fg(colors.semantic.warning),
		StatuslineRenderStyle::Error => Style::default().fg(colors.semantic.error),
		StatuslineRenderStyle::Success => Style::default().fg(colors.semantic.success),
	};
	Span::styled(segment.text.clone(), style)
}

pub fn render(ed: &Editor, frame: &mut xeno_tui::Frame, area: Rect) {
	let status_bg = Block::default().style(Style::default().bg(ed.config().theme.colors.ui.bg));
	frame.render_widget(status_bg, area);

	let spans: Vec<_> = ed.statusline_render_plan().iter().map(|segment| segment_to_span(ed, segment)).collect();
	frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
