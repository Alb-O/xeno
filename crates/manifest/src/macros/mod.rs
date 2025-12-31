//! Macros for registering editor primitives at compile time.
//!
//! These macros generate static entries in [`linkme`] distributed slices,
//! enabling zero-cost registration of actions, keybindings, motions, hooks,
//! and other extensible editor components.
//!
//! # Primary Macros
//!
//! - [`action!`] - Register actions with optional keybindings and handlers
//! - [`motion!`] - Cursor/selection movement primitives
//! - [`hook!`] - Event lifecycle observers
//! - [`command!`] - Ex-mode commands (`:write`, `:quit`)
//!
//! # Secondary Macros
//!
//! - [`option!`] - Configuration options
//! - [`text_object!`] - Text object selection (`iw`, `a"`, etc.)
//! - [`statusline_segment!`] - Statusline segment definitions
//!
//! Note: Language definitions are loaded at runtime from `languages.kdl`.

// Internal modules containing macro definitions
mod actions;
mod events;
mod helpers;
mod hooks;
mod panels;
mod registry;
mod text_objects;

// Re-export all macros at module level
pub use crate::{
	__async_hook_extract, __hook_borrowed_ty, __hook_extract, __hook_owned_ty, __hook_owned_value,
	__hook_param_expr, __opt, __opt_slice, action, async_hook, bracket_pair_object, command,
	events, hook, motion, option, panel, result_handler, statusline_segment, symmetric_text_object,
	text_object,
};
