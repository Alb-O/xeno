//! Serializable intermediate types for KDL-to-registry pipeline.

pub mod actions;
pub use actions::*;

pub mod commands;
pub use commands::*;

pub mod motions;
pub use motions::*;

pub mod textobj;
pub use textobj::*;

pub mod options;
pub use options::*;

pub mod gutter;
pub use gutter::*;

pub mod statusline;
pub use statusline::*;

pub mod hooks;
pub use hooks::*;

pub mod notifications;
pub use notifications::*;

pub mod themes;
pub use themes::*;
