use xeno_primitives::MouseEvent;

use crate::buffer::ViewId;
use crate::geometry::Rect;
use crate::impls::Editor;
use crate::layout::SeparatorHit;
use crate::separator::DragState;
use crate::window::WindowId;

#[derive(Debug, Clone, Copy)]
pub(super) struct OverlayHit {
	pub(super) buffer: ViewId,
	pub(super) inner: Rect,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ViewHit {
	pub(super) view: ViewId,
	pub(super) area: Rect,
	pub(super) window: WindowId,
}

#[derive(Debug, Clone)]
pub(super) struct MouseRouteContext {
	pub(super) mouse: MouseEvent,
	pub(super) doc_area: Rect,
	pub(super) mouse_x: u16,
	pub(super) mouse_y: u16,
	pub(super) active_drag: Option<DragState>,
	pub(super) text_selection_origin: Option<(ViewId, Rect)>,
	pub(super) overlay_hit: Option<OverlayHit>,
	pub(super) separator_hit: Option<SeparatorHit>,
	pub(super) view_hit: Option<ViewHit>,
}

impl Editor {
	pub(super) fn build_mouse_route_context(&self, mouse: MouseEvent, doc_area: Rect) -> MouseRouteContext {
		let mouse_x = mouse.col();
		let mouse_y = mouse.row();

		MouseRouteContext {
			mouse,
			doc_area,
			mouse_x,
			mouse_y,
			active_drag: self.state.core.layout.drag_state().cloned(),
			text_selection_origin: self.state.core.layout.text_selection_origin,
			overlay_hit: self.overlay_hit(mouse_x, mouse_y),
			separator_hit: self.separator_hit(doc_area, mouse_x, mouse_y),
			view_hit: self.view_hit(doc_area, mouse_x, mouse_y),
		}
	}

	fn overlay_hit(&self, mouse_x: u16, mouse_y: u16) -> Option<OverlayHit> {
		self.state.ui.overlay_system.interaction().active().and_then(|active| {
			active
				.session
				.panes
				.iter()
				.rev()
				.find(|pane| {
					mouse_x >= pane.rect.x
						&& mouse_x < pane.rect.x.saturating_add(pane.rect.width)
						&& mouse_y >= pane.rect.y
						&& mouse_y < pane.rect.y.saturating_add(pane.rect.height)
				})
				.map(|pane| OverlayHit {
					buffer: pane.buffer,
					inner: crate::overlay::geom::pane_inner_rect(pane.rect, &pane.style),
				})
		})
	}

	fn separator_hit(&self, doc_area: Rect, mouse_x: u16, mouse_y: u16) -> Option<SeparatorHit> {
		let base_layout = &self.base_window().layout;
		self.state.core.layout.separator_hit_at_position(base_layout, doc_area, mouse_x, mouse_y)
	}

	fn view_hit(&self, doc_area: Rect, mouse_x: u16, mouse_y: u16) -> Option<ViewHit> {
		let base_layout = &self.base_window().layout;
		self.state
			.core.layout
			.view_at_position(base_layout, doc_area, mouse_x, mouse_y)
			.map(|(view, area)| ViewHit {
				view,
				area,
				window: self.state.core.windows.base_id(),
			})
	}
}
