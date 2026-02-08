//! Gutter handler static registration via `inventory`.

use super::{GutterCell, GutterLineContext};

pub type GutterRenderHandler = fn(&GutterLineContext) -> Option<GutterCell>;
pub type GutterHandlerStatic = crate::core::HandlerStatic<GutterRenderHandler>;

/// Static handler registration entry collected via `inventory`.
/// Wrapper for `inventory::collect!`.
pub struct GutterHandlerReg(pub &'static GutterHandlerStatic);

inventory::collect!(GutterHandlerReg);
