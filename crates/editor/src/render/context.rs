use std::collections::HashMap;
use std::sync::Arc;

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
	diagnostics: HashMap<ViewId, Arc<DiagnosticLineMap>>,
	diagnostic_ranges: HashMap<ViewId, Arc<DiagnosticRangeMap>>,
}

impl LspRenderSnapshot {
	pub fn diagnostics_for(&self, buffer_id: ViewId) -> Option<&DiagnosticLineMap> {
		self.diagnostics.get(&buffer_id).map(|arc| arc.as_ref())
	}

	pub fn diagnostic_ranges_for(&self, buffer_id: ViewId) -> Option<&DiagnosticRangeMap> {
		self.diagnostic_ranges
			.get(&buffer_id)
			.map(|arc| arc.as_ref())
	}
}

impl Editor {
	/// Creates a render context with cached diagnostics.
	///
	/// Uses the provided render cache to avoid rebuilding diagnostic maps
	/// every frame. The cache is keyed by (DocumentId, diagnostics_epoch).
	pub fn render_ctx(&mut self) -> RenderCtx {
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

	/// Builds the LSP render snapshot using cached diagnostics.
	///
	/// Uses the diagnostics cache to avoid rebuilding maps every frame.
	/// The global diagnostics_version from the LSP layer serves as the epoch.
	#[cfg(feature = "lsp")]
	fn lsp_render_snapshot(&mut self) -> LspRenderSnapshot {
		use crate::lsp::diagnostics::{build_diagnostic_line_map, build_diagnostic_range_map};

		let mut snapshot = LspRenderSnapshot::default();

		// Get the global diagnostics epoch from LSP layer
		let epoch = self.state.lsp.diagnostics_version();

		for buffer in self.state.core.buffers.buffers() {
			let doc_id = buffer.document_id();

			let entry = self
				.state
				.render_cache
				.diagnostics
				.get_or_build(doc_id, epoch, || {
					let diagnostics = self.state.lsp.get_diagnostics(buffer);
					(
						build_diagnostic_line_map(&diagnostics),
						build_diagnostic_range_map(&diagnostics),
					)
				});

			snapshot
				.diagnostics
				.insert(buffer.id, entry.line_map.clone());
			snapshot
				.diagnostic_ranges
				.insert(buffer.id, entry.range_map.clone());
		}

		snapshot
	}

	/// Builds the LSP render snapshot using cached diagnostics (no-op without lsp feature).
	#[cfg(not(feature = "lsp"))]
	fn lsp_render_snapshot(&mut self) -> LspRenderSnapshot {
		LspRenderSnapshot::default()
	}
}
