//! Render text primitives used by core rendering.
//!
//! These structs are backend-neutral and can be adapted to toolkit primitives
//! by frontend crates.

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderLine<'a> {
	pub spans: Vec<RenderSpan<'a>>,
	pub style: Option<xeno_primitives::Style>,
}

impl<'a> RenderLine<'a> {
	pub fn style(mut self, style: xeno_primitives::Style) -> Self {
		self.style = Some(style);
		self
	}

	#[cfg(feature = "tui")]
	pub fn into_text_line(self) -> xeno_primitives::TextLine<'a> {
		let mut line = xeno_primitives::TextLine::from(self.spans.into_iter().map(RenderSpan::into_text_span).collect::<Vec<_>>());
		if let Some(style) = self.style {
			line = line.style(style);
		}
		line
	}
}

impl<'a> From<Vec<RenderSpan<'a>>> for RenderLine<'a> {
	fn from(spans: Vec<RenderSpan<'a>>) -> Self {
		Self { spans, style: None }
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSpan<'a> {
	pub content: std::borrow::Cow<'a, str>,
	pub style: xeno_primitives::Style,
}

impl<'a> RenderSpan<'a> {
	pub fn styled(content: impl Into<std::borrow::Cow<'a, str>>, style: xeno_primitives::Style) -> Self {
		Self {
			content: content.into(),
			style,
		}
	}

	#[cfg(feature = "tui")]
	pub fn into_text_span(self) -> xeno_primitives::TextSpan<'a> {
		xeno_primitives::TextSpan::styled(self.content, self.style)
	}
}
