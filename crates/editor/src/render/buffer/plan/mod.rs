//! Buffer viewport planning types and line-source abstractions.

pub mod line_source;
pub mod viewport;

pub use line_source::{LineSlice, LineSource};
pub use viewport::{RowKind, ViewportPlan, WrapAccess};
