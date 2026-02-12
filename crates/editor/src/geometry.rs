//! Shared geometry primitives for editor core/front-end boundaries.
//!
//! This module is the single import point for rectangle/position types used
//! across editor subsystems.
//!
//! Canonical geometry storage lives in `xeno-primitives`; frontend crates
//! convert to backend-specific geometry (for example `xeno_tui::layout`) at
//! render/event boundaries.

pub use xeno_primitives::geometry::{Position, Rect};
