//! Horizontal menu bar widget with dropdown submenus.

mod item;
mod layout;
mod state;
mod widget;

pub use item::{ICON_CELL_WIDTH, ICON_PADDING, ICON_TOTAL_WIDTH, MenuItem};
pub use layout::{DropdownLayout, MenuLayout};
pub use state::{MenuEvent, MenuState};
pub use widget::Menu;
