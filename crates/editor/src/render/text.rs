//! Render text primitives used by core rendering.
//!
//! These aliases centralize backend text coupling so the renderer can migrate
//! toward backend-neutral line/span data over time without wide call-site churn.

#[cfg(feature = "tui")]
pub type RenderLine<'a> = xeno_tui::text::Line<'a>;
#[cfg(feature = "tui")]
pub type RenderSpan<'a> = xeno_tui::text::Span<'a>;

#[cfg(not(feature = "tui"))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderLine<'a> {
	pub spans: Vec<RenderSpan<'a>>,
	pub style: Option<xeno_primitives::Style>,
}

#[cfg(not(feature = "tui"))]
impl<'a> RenderLine<'a> {
	pub fn style(mut self, style: xeno_primitives::Style) -> Self {
		self.style = Some(style);
		self
	}
}

#[cfg(not(feature = "tui"))]
impl<'a> From<Vec<RenderSpan<'a>>> for RenderLine<'a> {
	fn from(spans: Vec<RenderSpan<'a>>) -> Self {
		Self {
			spans,
			style: None,
		}
	}
}

#[cfg(not(feature = "tui"))]
#[derive(Debug, Clone, PartialEq)]
pub struct RenderSpan<'a> {
	pub content: std::borrow::Cow<'a, str>,
	pub style: xeno_primitives::Style,
}

#[cfg(not(feature = "tui"))]
impl<'a> RenderSpan<'a> {
	pub fn styled(content: impl Into<std::borrow::Cow<'a, str>>, style: xeno_primitives::Style) -> Self {
		Self {
			content: content.into(),
			style,
		}
	}
}
