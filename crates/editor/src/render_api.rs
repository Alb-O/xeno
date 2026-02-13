//! Frontend-facing render API boundary.
//!
//! This module re-exports the minimal render types/functions consumed by
//! frontend crates so coupling stays explicit and reviewable.

// Render plan types.
pub use crate::render::{
	BufferViewRenderPlan, DocumentRenderPlan, DocumentViewPlan, InfoPopupViewPlan, OverlayCompletionMenuTarget,
	OverlayPaneViewPlan, RenderLine, RenderSpan, SeparatorJunctionTarget, SeparatorRenderTarget, SeparatorScenePlan,
	SeparatorState,
};

// Completion types.
pub use crate::completion::{CompletionKind, CompletionRenderItem, CompletionRenderPlan};

// Snippet choice types.
pub use crate::snippet::{SnippetChoiceRenderItem, SnippetChoiceRenderPlan};

// Statusline types.
pub use crate::ui::{PanelRenderTarget, StatuslineRenderSegment, StatuslineRenderStyle};

// Panel identifiers.
pub use crate::ui::ids::UTILITY_PANEL_ID;

// Overlay types.
pub use crate::overlay::{OverlayControllerKind, OverlayPaneRenderTarget, WindowRole};

// Window/surface types.
pub use crate::window::SurfaceStyle;

// Info popup types.
pub use crate::info_popup::{InfoPopupId, InfoPopupRenderAnchor, InfoPopupRenderTarget};

// Buffer types.
pub use crate::buffer::{SplitDirection, ViewId};

// Geometry.
pub use crate::geometry::Rect;
