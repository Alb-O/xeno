pub mod dock;
mod focus;
pub mod ids;
pub mod keymap;
pub mod layer;
pub mod layers;
mod manager;
/// Panel traits and request types.
pub mod panel;
mod panels;
pub mod scene;

pub use focus::UiFocus;
pub use keymap::UiKeyChord;
pub use manager::{PanelRenderTarget, UiManager};
pub use panel::UiRequest;

pub(crate) fn utility_whichkey_desired_height(editor: &crate::impls::Editor) -> Option<u16> {
	panels::utility::UtilityPanel::whichkey_desired_height(editor)
}
