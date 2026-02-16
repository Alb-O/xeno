use xeno_primitives::{MouseEvent, ScrollDirection};

use super::context::{MouseRouteContext, OverlayHit, ViewHit};
use crate::buffer::ViewId;
use crate::geometry::Rect;
use crate::layout::SeparatorHit;
use crate::separator::DragState;

#[derive(Debug, Clone)]
pub(super) enum MouseRouteDecision {
	ContinueSeparatorDrag(DragState),
	EndSeparatorDrag,
	ContinueTextSelection {
		origin_view: ViewId,
		origin_area: Rect,
	},
	OverlayPane(OverlayHit),
	Document {
		separator_hit: Option<SeparatorHit>,
		view_hit: Option<ViewHit>,
	},
}

pub(super) fn decide_mouse_route(context: &MouseRouteContext) -> MouseRouteDecision {
	if let Some(drag_state) = context.active_drag.clone() {
		match context.mouse {
			MouseEvent::Drag { .. } => return MouseRouteDecision::ContinueSeparatorDrag(drag_state),
			MouseEvent::Release { .. } => return MouseRouteDecision::EndSeparatorDrag,
			_ => {}
		}
	}

	if let Some((origin_view, origin_area)) = context.text_selection_origin {
		match context.mouse {
			MouseEvent::Drag { .. }
			| MouseEvent::Scroll {
				direction: ScrollDirection::Up | ScrollDirection::Down,
				..
			} => {
				return MouseRouteDecision::ContinueTextSelection { origin_view, origin_area };
			}
			MouseEvent::Release { .. } => {}
			_ => {}
		}
	}

	if let Some(overlay_hit) = context.overlay_hit {
		return MouseRouteDecision::OverlayPane(overlay_hit);
	}

	MouseRouteDecision::Document {
		separator_hit: context.separator_hit.clone(),
		view_hit: context.view_hit,
	}
}
