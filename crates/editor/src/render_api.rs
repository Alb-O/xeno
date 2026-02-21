//! Frontend-facing render API boundary.
//!
//! This module re-exports the minimal render types/functions consumed by
//! frontend crates so coupling stays explicit and reviewable.

// Render plan types.
// Buffer types.
pub use crate::buffer::SplitDirection;
// Completion types.
pub use crate::completion::{CompletionKind, CompletionRenderItem, CompletionRenderPlan, FilePresentationRender};
// Geometry.
pub use crate::geometry::Rect;
// Info popup types.
pub use crate::info_popup::{InfoPopupId, InfoPopupRenderAnchor, InfoPopupRenderTarget};
// Overlay types.
pub use crate::overlay::{OverlayControllerKind, OverlayPaneRenderTarget, WindowRole};
pub use crate::render::{DocumentViewPlan, RenderLine, SeparatorJunctionTarget, SeparatorRenderTarget, SeparatorState};
// Snippet choice types.
pub use crate::snippet::{SnippetChoiceRenderItem, SnippetChoiceRenderPlan};
// Panel identifiers.
pub use crate::ui::ids::UTILITY_PANEL_ID;
// Statusline types.
pub use crate::ui::{PanelRenderTarget, StatuslineRenderSegment, StatuslineRenderStyle};
// Window/surface types.
pub use crate::window::SurfaceStyle;
