//! Window abstractions for docked and floating views.

mod floating;
mod manager;
mod types;

pub use manager::WindowManager;
pub use types::{BaseWindow, FloatingStyle, FloatingWindow, GutterSelector, Window, WindowId};
