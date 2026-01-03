#![warn(missing_docs)]
//! The `widgets` module contains the `Widget` and `StatefulWidget` traits, which are used to
//! render UI elements on the screen.

pub use self::block::{Block, BlockExt, Padding, TitlePosition};
pub use self::borders::{BorderType, Borders};
pub use self::clear::Clear;
pub use self::icon::Icon;
pub use self::keytree::{KeyTree, KeyTreeNode};
pub use self::list::{List, ListDirection, ListItem, ListState};
pub use self::menu::{Menu, MenuEvent, MenuItem, MenuState};
pub use self::paragraph::{Paragraph, Wrap};
pub use self::scrollbar::{Scrollbar, ScrollbarOrientation, ScrollbarState};
pub use self::stateful_widget::StatefulWidget;
pub use self::table::{Cell, HighlightSpacing, Row, Table, TableState};
pub use self::tabs::Tabs;
pub use self::widget::Widget;

/// Stateful widget trait for widgets with state.
mod stateful_widget;
/// Base widget trait.
mod widget;

pub mod block;
pub mod borders;
pub mod clear;
pub mod icon;
pub mod keytree;
pub mod list;
pub mod menu;
pub mod paragraph;
pub mod scrollbar;
pub mod table;
pub mod tabs;
pub mod terminal;

#[cfg(feature = "std")]
pub mod notifications;

#[cfg(not(feature = "std"))]
mod polyfills;
mod reflow;
