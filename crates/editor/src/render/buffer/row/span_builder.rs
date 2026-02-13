use xeno_primitives::Style;

use crate::render::RenderSpan;

#[derive(Debug, Default)]
pub struct SpanRunBuilder {
	spans: Vec<RenderSpan<'static>>,
	pending_style: Option<Style>,
	pending_text: String,
}

impl SpanRunBuilder {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn push_text(&mut self, style: Style, s: &str) {
		if s.is_empty() {
			return;
		}

		if let Some(pending) = self.pending_style {
			if pending == style {
				self.pending_text.push_str(s);
				return;
			} else {
				self.flush();
			}
		}

		self.pending_style = Some(style);
		self.pending_text.push_str(s);
	}

	pub fn push_spaces(&mut self, style: Style, n: usize) {
		if n == 0 {
			return;
		}
		self.push_text(style, &" ".repeat(n));
	}

	fn flush(&mut self) {
		if let Some(style) = self.pending_style.take() {
			let text = std::mem::take(&mut self.pending_text);
			if !text.is_empty() {
				self.spans.push(RenderSpan::styled(text, style));
			}
		}
	}

	pub fn finish(mut self) -> Vec<RenderSpan<'static>> {
		self.flush();
		self.spans
	}
}
