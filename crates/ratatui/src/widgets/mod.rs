#![warn(missing_docs)]
//! The `widgets` module contains the `Widget` and `StatefulWidget` traits, which are used to
//! render UI elements on the screen.

pub use self::block::{Block, BlockExt};
pub use self::borders::{BorderType, Borders};
pub use self::clear::Clear;
pub use self::list::{List, ListState};
pub use self::paragraph::Paragraph;
pub use self::stateful_widget::StatefulWidget;
pub use self::table::{Table, TableState};
pub use self::widget::Widget;

mod stateful_widget;
mod widget;

pub mod barchart;
pub mod block;
pub mod borders;
pub mod canvas;
pub mod chart;
pub mod clear;
pub mod gauge;
pub mod list;
pub mod logo;
pub mod mascot;
pub mod paragraph;
pub mod scrollbar;
pub mod sparkline;
pub mod table;
pub mod tabs;

#[cfg(not(feature = "std"))]
mod polyfills;
mod reflow;

#[cfg(feature = "calendar")]
pub mod calendar;
