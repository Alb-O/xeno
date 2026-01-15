//! Command palette for executing commands via floating input.
//!
//! The palette uses a scratch buffer as its input field, providing familiar
//! text editing controls. Commands are parsed and executed on Enter.

mod state;

pub use state::{Palette, PaletteState, palette_rect, palette_style};
