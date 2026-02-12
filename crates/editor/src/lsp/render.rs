//! LSP rendering support (diagnostic maps).

use super::system::LspSystem;
use crate::buffer::Buffer;
use crate::render::{DiagnosticLineMap, DiagnosticRangeMap};

#[cfg(feature = "lsp")]
impl LspSystem {
	#[allow(dead_code)]
	pub fn get_diagnostic_line_map(&self, buffer: &Buffer) -> DiagnosticLineMap {
		use crate::lsp::diagnostics::build_diagnostic_line_map;
		let diagnostics = self.get_diagnostics(buffer);
		build_diagnostic_line_map(&diagnostics)
	}

	#[allow(dead_code)]
	pub fn get_diagnostic_range_map(&self, buffer: &Buffer) -> DiagnosticRangeMap {
		use crate::lsp::diagnostics::build_diagnostic_range_map;
		let diagnostics = self.get_diagnostics(buffer);
		build_diagnostic_range_map(&diagnostics)
	}
}

#[cfg(not(feature = "lsp"))]
impl LspSystem {
	#[allow(dead_code)]
	pub fn get_diagnostic_line_map(&self, _buffer: &Buffer) -> DiagnosticLineMap {
		DiagnosticLineMap::new()
	}

	#[allow(dead_code)]
	pub fn get_diagnostic_range_map(&self, _buffer: &Buffer) -> DiagnosticRangeMap {
		DiagnosticRangeMap::new()
	}
}
