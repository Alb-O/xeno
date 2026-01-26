use std::collections::HashMap;

use xeno_registry::themes::Theme;
use xeno_tui::layout::Rect;

use super::buffer::{DiagnosticLineMap, DiagnosticRangeMap};
use crate::buffer::{Layout, SplitDirection, ViewId};
use crate::impls::Editor;
use crate::layout::LayoutManager;
use crate::types::Viewport;

pub struct RenderCtx {
	pub theme: Theme,
	pub viewport: Viewport,
	pub layout: LayoutSnapshot,
	pub lsp: LspRenderSnapshot,
}

#[derive(Clone, Copy, Debug)]
pub struct LayoutSnapshot {
	pub hovered_separator: Option<(SplitDirection, Rect)>,
	pub dragging_rect: Option<Rect>,
	pub animation_rect: Option<Rect>,
	pub animation_intensity: f32,
}

impl LayoutSnapshot {
	fn new(layout: &LayoutManager, base_layout: &Layout, viewport: Viewport) -> Self {
		let dragging_rect = viewport.doc_area.and_then(|doc_area| {
			layout
				.drag_state()
				.and_then(|drag| layout.separator_rect(base_layout, doc_area, &drag.id))
		});

		Self {
			hovered_separator: layout.hovered_separator,
			dragging_rect,
			animation_rect: layout.animation_rect(),
			animation_intensity: layout.animation_intensity(),
		}
	}
}

#[derive(Clone, Debug, Default)]
pub struct LspRenderSnapshot {
	diagnostics: HashMap<ViewId, DiagnosticLineMap>,
	diagnostic_ranges: HashMap<ViewId, DiagnosticRangeMap>,
}

impl LspRenderSnapshot {
	pub fn diagnostics_for(&self, buffer_id: ViewId) -> Option<&DiagnosticLineMap> {
		self.diagnostics.get(&buffer_id)
	}

	pub fn diagnostic_ranges_for(&self, buffer_id: ViewId) -> Option<&DiagnosticRangeMap> {
		self.diagnostic_ranges.get(&buffer_id)
	}
}

impl Editor {
	pub fn render_ctx(&self) -> RenderCtx {
		RenderCtx {
			theme: *self.state.config.theme,
			viewport: self.state.viewport,
			layout: LayoutSnapshot::new(
				&self.state.layout,
				&self.base_window().layout,
				self.state.viewport,
			),
			lsp: self.lsp_render_snapshot(),
		}
	}

	fn lsp_render_snapshot(&self) -> LspRenderSnapshot {
		let mut snapshot = LspRenderSnapshot::default();
		for buffer in self.state.core.buffers.buffers() {
			snapshot
				.diagnostics
				.insert(buffer.id, self.state.lsp.get_diagnostic_line_map(buffer));
			snapshot
				.diagnostic_ranges
				.insert(buffer.id, self.state.lsp.get_diagnostic_range_map(buffer));
		}
		snapshot
	}
}
