pub mod dock;
mod focus;
pub mod ids;
pub mod keymap;
mod manager;
/// Panel traits and request types.
pub mod panel;
mod panels;
mod statusline;

pub use focus::UiFocus;
pub use keymap::UiKeyChord;
pub use manager::{PanelRenderTarget, UiManager};
pub use panel::UiRequest;
pub use panels::utility::{UtilityWhichKeyEntry, UtilityWhichKeyPlan};
pub use statusline::{StatuslineRenderSegment, StatuslineRenderStyle};

pub(crate) fn utility_whichkey_desired_height(editor: &crate::Editor) -> Option<u16> {
	panels::utility::UtilityPanel::whichkey_desired_height(editor)
}

pub(crate) fn utility_whichkey_render_plan(editor: &crate::Editor) -> Option<UtilityWhichKeyPlan> {
	panels::utility::UtilityPanel::whichkey_render_plan(editor)
}

pub(crate) fn statusline_render_plan(editor: &crate::Editor) -> Vec<StatuslineRenderSegment> {
	statusline::render_plan(editor)
}

pub(crate) fn statusline_segment_style(editor: &crate::Editor, style: StatuslineRenderStyle) -> xeno_primitives::Style {
	statusline::segment_style(editor, style)
}
