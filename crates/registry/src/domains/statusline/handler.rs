//! Statusline segment handler static registration via `inventory`.

use super::{RenderedSegment, StatuslineContext};

pub type StatuslineRenderHandler = fn(&StatuslineContext) -> Option<RenderedSegment>;
pub type StatuslineHandlerStatic = crate::core::HandlerStatic<StatuslineRenderHandler>;

/// Static handler registration entry collected via `inventory`.
/// Wrapper for `inventory::collect!`.
pub struct StatuslineHandlerReg(pub &'static StatuslineHandlerStatic);

inventory::collect!(StatuslineHandlerReg);
