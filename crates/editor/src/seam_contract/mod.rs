//! Build-time seam enforcement: ensures frontend crates only access core editor
//! types through `render_api` and never reach into internal modules directly.

/// Forbidden substring patterns in frontend source files.
///
/// Each entry is `(pattern, reason)`. If any frontend `.rs` file contains the
/// pattern, the test fails with the reason and file location.
const FORBIDDEN_PATTERNS: &[(&str, &str)] = &[
	// Direct module imports that should go through render_api.
	("xeno_editor::completion::", "use render_api re-exports instead of xeno_editor::completion"),
	("xeno_editor::snippet::", "use render_api re-exports instead of xeno_editor::snippet"),
	("xeno_editor::overlay::", "use render_api re-exports instead of xeno_editor::overlay"),
	("xeno_editor::ui::", "use render_api re-exports instead of xeno_editor::ui"),
	("xeno_editor::info_popup::", "use render_api re-exports instead of xeno_editor::info_popup"),
	("xeno_editor::window::", "use render_api re-exports instead of xeno_editor::window"),
	("xeno_editor::geometry::", "use render_api re-exports instead of xeno_editor::geometry"),
	// Internal render module (frontends should use render_api, not render::).
	("xeno_editor::render::", "use render_api re-exports instead of xeno_editor::render"),
	// Policy/internal calls that should not appear in frontends.
	("buffer_view_render_plan", "use core-owned view plan APIs instead"),
	("editor.layout(", "use core-owned separator/document plan APIs instead"),
	(".layout(", "use core-owned separator/document plan APIs instead"),
	(".layout_mut(", "use core-owned separator/document plan APIs instead"),
	("base_window().layout", "use core-owned plan APIs instead"),
	(".ui_mut(", "use core-owned frame planning APIs instead"),
	(".viewport_mut(", "use core-owned frame planning APIs instead"),
	(".frame_mut(", "use core-owned frame planning APIs instead"),
	// Legacy single-document render plan (replaced by document_view_plans).
	("DocumentRenderPlan", "use document_view_plans() instead"),
	("focused_document_render_plan", "use document_view_plans() instead"),
	// Legacy drift vectors.
	("BufferRenderContext", "internal render type leaked to frontend"),
	("RenderBufferParams", "internal render type leaked to frontend"),
];

/// Frontend source directories to scan, relative to this crate's manifest dir.
const FRONTEND_DIRS: &[&str] = &["../frontend_tui/src", "../frontend_iced/src"];

#[cfg(test)]
mod tests;
