use ratatui::widgets::Paragraph;

/// A segment of a wrapped line.
pub struct WrapSegment {
	pub text: String,
	pub start_offset: usize,
}

pub struct RenderResult {
	pub widget: Paragraph<'static>,
}
