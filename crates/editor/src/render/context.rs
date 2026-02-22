use std::collections::HashMap;
use std::sync::Arc;

use xeno_registry::themes::Theme;

use super::{DiagnosticLineMap, DiagnosticRangeMap, InlayHintRangeMap};
use crate::Editor;
use crate::buffer::{Layout, SplitDirection, ViewId};
use crate::geometry::Rect;
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
		let dragging_rect = viewport
			.doc_area
			.and_then(|doc_area| layout.drag_state().and_then(|drag| layout.separator_rect(base_layout, doc_area, &drag.id)));

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
	inlay_hints: HashMap<ViewId, Arc<InlayHintRangeMap>>,
	#[cfg(feature = "lsp")]
	semantic_tokens: HashMap<ViewId, Arc<crate::lsp::semantic_tokens::SemanticTokenSpans>>,
	#[cfg(feature = "lsp")]
	document_highlights: HashMap<ViewId, crate::lsp::document_highlight::DocumentHighlightSpans>,
}

impl LspRenderSnapshot {
	pub fn diagnostics_for(&self, buffer_id: ViewId) -> Option<&DiagnosticLineMap> {
		self.diagnostics.get(&buffer_id).map(|arc| arc.as_ref())
	}

	pub fn diagnostic_ranges_for(&self, buffer_id: ViewId) -> Option<&DiagnosticRangeMap> {
		self.diagnostic_ranges.get(&buffer_id).map(|arc| arc.as_ref())
	}

	pub fn inlay_hints_for(&self, buffer_id: ViewId) -> Option<&InlayHintRangeMap> {
		self.inlay_hints.get(&buffer_id).map(|arc| arc.as_ref())
	}

	#[cfg(feature = "lsp")]
	pub fn semantic_tokens_for(&self, buffer_id: ViewId) -> Option<&crate::lsp::semantic_tokens::SemanticTokenSpans> {
		self.semantic_tokens.get(&buffer_id).map(|arc| arc.as_ref())
	}

	#[cfg(feature = "lsp")]
	pub fn document_highlights_for(&self, buffer_id: ViewId) -> Option<&crate::lsp::document_highlight::DocumentHighlightSpans> {
		self.document_highlights.get(&buffer_id)
	}
}

impl Editor {
	/// Creates a render context with cached diagnostics.
	///
	/// Uses the provided render cache to avoid rebuilding diagnostic maps
	/// every frame. The cache is keyed by (DocumentId, diagnostics_epoch).
	pub fn render_ctx(&mut self) -> RenderCtx {
		RenderCtx {
			theme: self.state.config.config.theme,
			viewport: self.state.core.viewport,
			layout: LayoutSnapshot::new(&self.state.core.layout, &self.base_window().layout, self.state.core.viewport),
			lsp: self.lsp_render_snapshot(),
		}
	}

	/// Builds the LSP render snapshot using cached diagnostics.
	///
	/// Uses the diagnostics cache to avoid rebuilding maps every frame.
	/// The global diagnostics version from the LSP layer serves as the epoch.
	///
	/// Diagnostics are only fetched and processed on cache misses to ensure
	/// high performance in the render loop.
	#[cfg(feature = "lsp")]
	fn lsp_render_snapshot(&mut self) -> LspRenderSnapshot {
		use crate::lsp::diagnostics::{build_diagnostic_line_map, build_diagnostic_range_map};

		let mut snapshot = LspRenderSnapshot::default();
		let epoch = self.state.integration.lsp.diagnostics_version();

		for buffer in self.state.core.editor.buffers.buffers() {
			let doc_id = buffer.document_id();

			let entry = self.state.ui.render_cache.diagnostics.get_or_build(doc_id, epoch, || {
				let diagnostics = self.state.integration.lsp.get_diagnostics(buffer);
				(build_diagnostic_line_map(&diagnostics), build_diagnostic_range_map(&diagnostics))
			});

			snapshot.diagnostics.insert(buffer.id, entry.line_map.clone());
			snapshot.diagnostic_ranges.insert(buffer.id, entry.range_map.clone());

			{
				let doc_rev = buffer.version();
				let line_lo = buffer.scroll_line;
				let viewport_height = self.state.core.viewport.height.unwrap_or(24) as usize;
				let line_hi = line_lo + viewport_height + 2;
				if let Some(hints) = self.state.ui.inlay_hint_cache.get(buffer.id, doc_rev, line_lo, line_hi) {
					snapshot.inlay_hints.insert(buffer.id, hints.clone());
				}
				if let Some(tokens) = self.state.ui.semantic_token_cache.get(buffer.id, doc_rev, line_lo, line_hi) {
					snapshot.semantic_tokens.insert(buffer.id, tokens.clone());
				}
				if let Some(highlights) = self.state.ui.document_highlight_cache.get_for_render(
					buffer.id,
					doc_rev,
					buffer.cursor,
					crate::lsp::document_highlight::DOCUMENT_HIGHLIGHT_SETTLE_TICKS,
				) {
					snapshot.document_highlights.insert(buffer.id, highlights.clone());
				}
			}
		}

		snapshot
	}

	/// Builds the LSP render snapshot using cached diagnostics (no-op without lsp feature).
	#[cfg(not(feature = "lsp"))]
	fn lsp_render_snapshot(&mut self) -> LspRenderSnapshot {
		LspRenderSnapshot::default()
	}
}
