pub mod capabilities;
pub mod editor;
pub mod ipc;
pub mod paths;
pub mod render;
pub mod styles;
pub mod terminal_panel;
pub mod ui;

pub use editor::Editor;
pub use tome_theme::{
	PopupColors, StatusColors, THEMES, Theme, ThemeColors, UiColors, blend_colors, get_theme,
	suggest_theme,
};
pub use ui::UiManager;
