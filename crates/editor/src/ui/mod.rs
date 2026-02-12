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

pub(crate) fn utility_whichkey_desired_height(editor: &crate::impls::Editor) -> Option<u16> {
	panels::utility::UtilityPanel::whichkey_desired_height(editor)
}

pub(crate) fn utility_whichkey_render_plan(editor: &crate::impls::Editor) -> Option<UtilityWhichKeyPlan> {
	panels::utility::UtilityPanel::whichkey_render_plan(editor)
}

pub(crate) fn statusline_render_plan(editor: &crate::impls::Editor) -> Vec<StatuslineRenderSegment> {
	statusline::render_plan(editor)
}
