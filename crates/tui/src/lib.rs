#![cfg_attr(test, allow(unused_crate_dependencies))]
//! TUI library for building terminal user interfaces.
//!
//! Provides widgets, layout primitives, and a rendering pipeline for terminal applications.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![allow(
	clippy::module_inception,
	unfulfilled_lint_expectations,
	reason = "module_inception is intentional for re-exports; unfulfilled_lint_expectations may vary by feature flags"
)]

pub mod animation;
pub mod backend;
pub mod buffer;
pub mod layout;
pub mod macros;
pub mod style;
pub mod symbols;
pub mod terminal;
pub mod text;
pub mod widgets;

/// re-export the `palette` crate so that users don't have to add it as a dependency
#[cfg(feature = "palette")]
pub use palette;

pub use crate::terminal::{CompletedFrame, Frame, Terminal, TerminalOptions, Viewport};

/// The `core` module provides a re-export of the library's items for convenience.
pub mod core {
	pub use crate::*;
}
