pub mod dock;
mod focus;
pub mod keymap;
mod manager;
pub mod panel;
pub mod panels;
mod split_buffer_panel;

pub use focus::FocusTarget;
pub use keymap::UiKeyChord;
pub use manager::UiManager;
pub use panel::UiRequest;
pub use split_buffer_panel::{SplitBufferPanel, SplitBufferPanelConfig};
