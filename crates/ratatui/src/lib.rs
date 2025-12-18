//! Ratatui is a library for building terminal user interfaces in Rust.
//!
//! It is a lightweight library that provides a set of widgets and utilities to build complex
//! terminal user interfaces.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![allow(clippy::module_inception, unfulfilled_lint_expectations)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod backend;
pub mod buffer;
pub mod init;
pub mod layout;
pub mod macros;
pub mod prelude;
pub mod style;
pub mod symbols;
pub mod terminal;
pub mod text;
pub mod widgets;

/// re-export the `palette` crate so that users don't have to add it as a dependency
#[cfg(feature = "palette")]
pub use palette;

#[cfg(feature = "crossterm")]
pub use crate::backend::crossterm::crossterm;
#[cfg(feature = "crossterm")]
pub use crate::init::{
	DefaultTerminal, init, init_with_options, restore, run, try_init, try_init_with_options,
	try_restore,
};
pub use crate::terminal::{CompletedFrame, Frame, Terminal, TerminalOptions, Viewport};

/// The `core` module provides a re-export of the library's items for convenience.
pub mod core {
	pub use crate::*;
}

#[doc(hidden)]
pub use alloc::{format, vec};
