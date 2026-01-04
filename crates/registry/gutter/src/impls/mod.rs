//! Built-in gutter column implementations.
//!
//! # Enabled by Default
//!
//! - [`line_numbers`] - Absolute line numbers (priority 0)
//!
//! # Disabled by Default
//!
//! - [`relative_line_numbers`] - Distance from cursor (priority 0)
//! - [`hybrid_line_numbers`] - Absolute on cursor, relative elsewhere (priority 0)
//! - [`signs`] - Sign column for diagnostics/markers (priority -10)
//!
//! Note: `line_numbers`, `relative_line_numbers`, and `hybrid_line_numbers` all
//! have priority 0 and are mutually exclusive. Only one should be enabled at a time.

mod hybrid;
mod line_numbers;
mod relative;
mod signs;
