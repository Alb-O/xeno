//! Render text primitives used by core rendering.
//!
//! These aliases centralize backend text coupling so the renderer can migrate
//! toward backend-neutral line/span data over time without wide call-site churn.

pub type RenderLine<'a> = xeno_tui::text::Line<'a>;
pub type RenderSpan<'a> = xeno_tui::text::Span<'a>;
