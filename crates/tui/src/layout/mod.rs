#![warn(clippy::missing_const_for_fn)]
//! Layout and positioning in terminal user interfaces.
//!
//! This module provides a comprehensive set of types and traits for working with layout and
//! positioning in terminal applications. It implements a flexible layout system that allows you to
//! divide the terminal screen into different areas using constraints, manage positioning and
//! sizing, and handle complex UI arrangements.
//!
//! The layout system uses a deterministic three-pass algorithm that allocates space in priority
//! order: `Length` (exact), `Percentage` (proportional), then `Min` (remainder). This provides
//! predictable, cache-free layout results.
//!
//! # Core Concepts
//!
//! ## Coordinate System
//!
//! The coordinate system runs left to right, top to bottom, with the origin `(0, 0)` in the top
//! left corner of the terminal. The x and y coordinates are represented by `u16` values.
//!
//! ```text
//!      x (columns)
//!   ┌─────────────→
//! y │ (0,0)
//!   │
//! (rows)
//!   ↓
//! ```
//!
//! ## Layout Fundamentals
//!
//! Layouts form the structural foundation of your terminal UI. The [`Layout`] struct divides
//! available screen space into rectangular areas using a constraint-based approach. You define
//! multiple constraints for how space should be allocated, and the solver determines the layout
//! deterministically. These areas can then be used to render widgets or nested layouts.
//!
//! Note that the [`Layout`] struct is not required to create layouts - you can also manually
//! calculate and create [`Rect`] areas using simple mathematics to divide up the terminal space
//! if you prefer direct control over positioning and sizing.
//!
//! ## Rectangular Areas
//!
//! All layout operations work with rectangular areas represented by the [`Rect`] type. A [`Rect`]
//! defines a position and size in the terminal, specified by its top-left corner coordinates and
//! dimensions.
//!
//! # Available Types
//!
//! ## Core Layout Types
//!
//! - [`Layout`] - The primary layout engine that divides space using constraints and direction
//! - [`Rect`] - Represents a rectangular area with position and dimensions
//! - [`Constraint`] - Defines how space should be allocated (length, percentage, min)
//! - [`Direction`] - Specifies layout orientation (horizontal or vertical)
//!
//! ## Positioning and Sizing
//!
//! - [`Position`] - Represents a point in the terminal coordinate system
//! - [`Size`] - Represents dimensions (width and height)
//! - [`Margin`] - Defines spacing around rectangular areas
//! - [`Offset`] - Represents relative movement in the coordinate system
//!
//! ## Alignment
//!
//! - [`HorizontalAlignment`] - Horizontal alignment options (left, center, right)
//! - [`VerticalAlignment`] - Vertical alignment options (top, center, bottom)
//!
//! ## Iteration Support
//!
//! - [`Rows`] - Iterator over horizontal rows within a rectangular area
//! - [`Columns`] - Iterator over vertical columns within a rectangular area
//! - [`Positions`] - Iterator over all positions within a rectangular area
//!
//! # Quick Start
//!
//! Here's a simple example of creating a basic layout using the [`Layout`] struct:
//!
//! ```rust
//! use xeno_tui::layout::{Constraint, Direction, Layout, Rect};
//!
//! // Create a terminal area
//! let area = Rect::new(0, 0, 80, 24);
//!
//! // Divide it vertically into two equal parts using Layout
//! let layout = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]);
//! let [top, bottom] = layout.areas(area);
//!
//! // Now you have two areas: top and bottom
//! ```
//!
//! When the number of layout areas is known at compile time, use destructuring
//! assignment with descriptive variable names for better readability:
//!
//! ```rust
//! use xeno_tui::layout::{Constraint, Layout, Rect};
//!
//! let area = Rect::new(0, 0, 80, 24);
//! let [header, content, footer] = Layout::vertical([
//!     Constraint::Length(3),
//!     Constraint::Min(1),
//!     Constraint::Length(1),
//! ])
//! .areas(area);
//! ```
//!
//! Use [`Layout::split`] when the number of areas is only known at runtime.
//!
//! Alternatively, you can create layouts manually using mathematics:
//!
//! ```rust
//! use xeno_tui::layout::Rect;
//!
//! // Create a terminal area
//! let area = Rect::new(0, 0, 80, 24);
//!
//! // Manually divide into two equal parts
//! let top_half = Rect::new(area.x, area.y, area.width, area.height / 2);
//! let bottom_half = Rect::new(
//!     area.x,
//!     area.y + area.height / 2,
//!     area.width,
//!     area.height / 2,
//! );
//! ```
//!
//! # Layout Examples
//!
//! ## Basic Vertical Split
//!
//! ```rust
//! use xeno_tui::layout::{Constraint, Layout, Rect};
//!
//! let area = Rect::new(0, 0, 80, 24);
//! let [header, content, footer] = Layout::vertical([
//!     Constraint::Length(3), // Header: fixed height
//!     Constraint::Min(1),   // Content: flexible
//!     Constraint::Length(1), // Footer: fixed height
//! ])
//! .areas(area);
//! ```
//!
//! ## Horizontal Sidebar Layout
//!
//! ```rust
//! use xeno_tui::layout::{Constraint, Layout, Rect};
//!
//! let area = Rect::new(0, 0, 80, 24);
//! let [sidebar, main] = Layout::horizontal([
//!     Constraint::Length(20), // Sidebar: fixed width
//!     Constraint::Min(1),    // Main content: flexible
//! ])
//! .areas(area);
//! ```
//!
//! ## Complex Nested Layout
//!
//! ```rust
//! use xeno_tui::layout::{Constraint, Layout, Rect};
//!
//! fn create_complex_layout(area: Rect) -> [Rect; 4] {
//!     // First, split vertically
//!     let [header, body, footer] = Layout::vertical([
//!         Constraint::Length(3), // Header
//!         Constraint::Min(1),   // Body
//!         Constraint::Length(1), // Footer
//!     ])
//!     .areas(area);
//!
//!     // Then split the body horizontally
//!     let [sidebar, main] = Layout::horizontal([
//!         Constraint::Length(20), // Sidebar
//!         Constraint::Min(1),    // Main
//!     ])
//!     .areas(body);
//!
//!     [header, sidebar, main, footer]
//! }
//! ```
//!
//! # Working with Constraints
//!
//! [`Constraint`]s define how space is allocated within a layout. The deterministic solver
//! allocates space in priority passes. Different constraint types serve different purposes:
//!
//! - [`Constraint::Length`] - Fixed size in character cells (allocated first)
//! - [`Constraint::Percentage`] - Relative size as a percentage of total space (allocated second)
//! - [`Constraint::Min`] - Minimum size, receives remaining space (allocated last)
//!
//! # Positioning and Alignment
//!
//! Use [`Position`] to represent specific points in the terminal, [`Size`] for dimensions, and the
//! alignment types for controlling content positioning within areas:
//!
//! ```rust
//! use xeno_tui::layout::{HorizontalAlignment, Position, Rect, Size};
//!
//! let pos = Position::new(10, 5);
//! let size = Size::new(80, 24);
//! let rect = Rect::new(pos.x, pos.y, size.width, size.height);
//!
//! // Alignment for content within areas
//! let center = HorizontalAlignment::Center;
//! ```
//!
//! # Advanced Features
//!
//! ## Margins
//!
//! Add spacing around areas using margins:
//!
//! ```rust
//! use xeno_tui::layout::{Margin, Rect};
//!
//! // For asymmetric margins, use the Rect inner method directly
//! let area = Rect::new(0, 0, 80, 24).inner(Margin::new(2, 1));
//! ```
//!
//! ## Area Iteration
//!
//! Iterate over rows, columns, or all positions within a rectangular area. The `rows()` and
//! `columns()` iterators return full [`Rect`] regions that can be used to render widgets or
//! passed to other layout methods for more complex nested layouts. The `positions()` iterator
//! returns [`Position`] values representing individual cell coordinates:
//!
//! ```rust
//! use xeno_tui::buffer::Buffer;
//! use xeno_tui::layout::{Constraint, Layout, Rect};
//! use xeno_tui::widgets::Widget;
//!
//! let area = Rect::new(0, 0, 20, 10);
//! let mut buffer = Buffer::empty(area);
//!
//! // Renders "Row 0", "Row 1", etc. in each horizontal row
//! for (i, row) in area.rows().enumerate() {
//!     format!("Row {i}").render(row, &mut buffer);
//! }
//!
//! // Renders column indices (0-9 repeating) in each vertical column
//! for (i, col) in area.columns().enumerate() {
//!     format!("{}", i % 10).render(col, &mut buffer);
//! }
//!
//! // Renders position indices (0-9 repeating) at each cell position
//! for (i, pos) in area.positions().enumerate() {
//!     buffer[pos].set_symbol(&format!("{}", i % 10));
//! }
//! ```

/// Horizontal and vertical alignment types for positioning content within areas.
mod alignment;
/// Size constraints for layout calculations (length, percentage, min).
mod constraint;
/// Layout direction (horizontal or vertical).
mod direction;
/// Core layout engine that divides space using constraints.
mod engine;
/// Margin definitions for spacing around rectangular areas.
mod margin;
/// Offset type for relative positioning.
mod offset;
/// Position type representing a point in the terminal coordinate system.
mod position;
/// Rectangular area with position and dimensions.
mod rect;
/// Size type representing width and height dimensions.
mod size;

pub use alignment::{HorizontalAlignment, VerticalAlignment};
pub use constraint::Constraint;
pub use direction::Direction;
pub use engine::Layout;
pub use margin::Margin;
pub use offset::Offset;
pub use position::Position;
pub use rect::{Columns, Positions, Rect, Rows};
pub use size::Size;
