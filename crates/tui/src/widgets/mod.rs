#![warn(missing_docs)]
//! The `widgets` module contains the `Widget` and `StatefulWidget` traits, which are used to
//! render UI elements on the screen.

pub use self::barchart::{Bar, BarChart, BarGroup};
pub use self::block::{Block, BlockExt, Padding, TitlePosition};
pub use self::borders::{BorderType, Borders};
pub use self::chart::{Axis, Chart, Dataset, GraphType, LegendPosition};
pub use self::clear::Clear;
pub use self::gauge::{Gauge, LineGauge};
pub use self::icon::Icon;
pub use self::list::{List, ListDirection, ListItem, ListState};
pub use self::paragraph::{Paragraph, Wrap};
pub use self::scrollbar::{Scrollbar, ScrollbarOrientation, ScrollbarState};
pub use self::sparkline::{RenderDirection, Sparkline, SparklineBar};
pub use self::stateful_widget::StatefulWidget;
pub use self::table::{Cell, HighlightSpacing, Row, Table, TableState};
pub use self::tabs::Tabs;
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
pub mod icon;
pub mod list;
pub mod paragraph;
pub mod scrollbar;
pub mod sparkline;
pub mod table;
pub mod tabs;
pub mod terminal;

#[cfg(feature = "std")]
pub mod notifications;

#[cfg(not(feature = "std"))]
mod polyfills;
mod reflow;

#[cfg(feature = "calendar")]
pub mod calendar;
