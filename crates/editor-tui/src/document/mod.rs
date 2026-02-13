//! Document rendering logic for the editor.
//!
//! This module handles rendering of buffers in split views, including
//! separator styling and junction glyphs.

mod separator;

use xeno_editor::Editor;
use xeno_editor::render_api::SplitDirection;
use xeno_tui::layout::Rect;
use xeno_tui::text::{Line, Span};
use xeno_tui::widgets::Paragraph;

use self::separator::SeparatorStyle;
use crate::render_adapter::to_tui_lines;

/// Renders all views and separators across all layout layers.
///
/// Consumes core-owned document view plans, separator render targets, and
/// junction targets. Frontends only draw; core decides focus, cursor, gutter,
/// layout, and junction glyphs.
pub fn render_split_buffers(ed: &mut Editor, frame: &mut xeno_tui::Frame, doc_area: Rect) {
	let doc_plans = ed.document_view_plans(doc_area.into());
	let sep_scene = ed.separator_scene_plan(doc_area.into());
	let sep_targets = sep_scene.separators();
	let junction_targets = sep_scene.junctions();

	for plan in &doc_plans {
		let gutter_area: Rect = plan.gutter_rect().into();
		let text_area: Rect = plan.text_rect().into();

		let gutter = to_tui_lines(plan.gutter().to_vec());
		let text = to_tui_lines(plan.text().to_vec());

		frame.render_widget(Paragraph::new(gutter), gutter_area);
		frame.render_widget(Paragraph::new(text), text_area);
	}

	let sep_style = SeparatorStyle::new(ed, sep_targets);

	for target in sep_targets {
		let rect: Rect = target.rect().into();
		let style = sep_style.for_target(target);
		let lines: Vec<Line> = match target.direction() {
			SplitDirection::Horizontal => (0..rect.height).map(|_| Line::from(Span::styled("\u{2502}", style))).collect(),
			SplitDirection::Vertical => vec![Line::from(Span::styled("\u{2500}".repeat(rect.width as usize), style))],
		};
		frame.render_widget(Paragraph::new(lines), rect);
	}

	// Junction glyphs from core.
	let buf = frame.buffer_mut();
	for junc in junction_targets {
		let style = sep_style.for_junction(junc.x(), junc.y(), junc.priority());
		if let Some(cell) = buf.cell_mut((junc.x(), junc.y())) {
			cell.set_char(junc.glyph());
			cell.set_style(style);
		}
	}
}
