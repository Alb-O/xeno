pub mod capabilities;
pub mod editor;
pub mod ipc;
pub mod paths;
pub mod render;
pub mod styles;
pub mod terminal_panel;
pub mod theme;
pub mod themes;
pub mod ui;

pub use editor::Editor;
pub use theme::{PopupColors, StatusColors, Theme, ThemeColors, UiColors};
pub use ui::UiManager;
