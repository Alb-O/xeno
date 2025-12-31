//! Input handling for the editor.
//!
//! Processing keyboard and mouse events:
//!
//! - [`key_handling`] - Keyboard input and action dispatch
//! - [`mouse_handling`] - Mouse events for selection and navigation
//! - [`panel_input`] - Routing input to focused panels
//! - [`conversions`] - Terminal event type conversions

mod conversions;
mod key_handling;
mod mouse_handling;
mod panel_input;
