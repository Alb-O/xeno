//! Pseudoterminal widget for rendering terminal emulator output.
//!
//! This module provides the [`PseudoTerminal`] widget for displaying the contents of a
//! pseudoterminal screen within a TUI application. It uses the `vt100` crate for parsing
//! and processing terminal control sequences.
//!
//! # Example
//!
//! ```rust,ignore
//! use evildoer_tui::{
//!     style::{Color, Modifier, Style},
//!     widgets::{Block, Borders},
//! };
//! use evildoer_tui::widgets::terminal::PseudoTerminal;
//! use vt100::Parser;
//!
//! let mut parser = vt100::Parser::new(24, 80, 0);
//! let pseudo_term = PseudoTerminal::new(parser.screen())
//!     .block(Block::default().title("Terminal").borders(Borders::ALL))
//!     .style(
//!         Style::default()
//!             .fg(Color::White)
//!             .bg(Color::Black)
//!             .add_modifier(Modifier::BOLD),
//!     );
//! ```

/// Terminal state management.
mod state;
/// vt100 screen conversion.
mod vt100_impl;
/// Terminal widget implementation.
mod widget;

pub use widget::{Cell, Cursor, PseudoTerminal, Screen};
