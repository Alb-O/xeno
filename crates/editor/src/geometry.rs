//! Shared geometry primitives for editor core/front-end boundaries.
//!
//! This module is the single import point for rectangle/position types used
//! across editor subsystems. It currently aliases `xeno_tui` geometry and is
//! intentionally narrow so we can swap backend dependencies in one place.

pub type Rect = xeno_tui::layout::Rect;
pub type Position = xeno_tui::layout::Position;
